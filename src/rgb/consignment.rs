use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    iter,
};

use amplify::ByteArray;
use bitcoin_30::{hashes::Hash, psbt::Psbt};
use bp::{Outpoint, Txid};
use chrono::Utc;
use rgbstd::{
    containers::{Bindle, BuilderSeal, Transfer},
    contract::{ContractId, GraphSeal, Operation, Opout, SecretSeal},
    interface::{BuilderError, ContractSuppl, TypedState, VelocityHint},
    persistence::{Inventory, Stash},
    schema::AssignmentType,
};
use rgbwallet::{
    psbt::{PsbtDbc, RgbExt, RgbInExt, RgbOutExt},
    Beneficiary, PayError, RgbInvoice,
};
use seals::txout::CloseMethod;

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub struct PreviousSeal {
    pub seal: SecretSeal,
    pub state: TypedState,
}

impl PreviousSeal {
    pub fn with(seal: SecretSeal, state: TypedState) -> Self {
        Self { seal, state }
    }
}

#[derive(Clone, Debug, Display, Error, From, Default)]
#[display(doc_comments)]
pub struct NewTransferOptions {
    pub strict: bool,
    pub other_invoices: Vec<RgbInvoice>,
    pub offer_id: Option<String>,
    pub bid_id: Option<String>,
}

impl NewTransferOptions {
    pub fn with(strict: bool, other_invoices: Vec<RgbInvoice>) -> Self {
        Self {
            strict,
            other_invoices,
            ..Default::default()
        }
    }
}

pub trait ConsignmentEx: Inventory {
    /// # Assumptions
    ///
    /// 1. If PSBT output has BIP32 derivation information it belongs to our
    /// wallet - except when it matches address from the invoice.
    #[allow(clippy::result_large_err, clippy::type_complexity)]
    fn process(
        &mut self,
        invoice: RgbInvoice,
        psbt: &mut Psbt,
        method: CloseMethod,
        options: NewTransferOptions,
    ) -> Result<Vec<Bindle<Transfer>>, PayError<Self::Error, <Self::Stash as Stash>::Error>>
    where
        Self::Error: From<<Self::Stash as Stash>::Error>,
    {
        // 1. Prepare the data
        if let Some(expiry) = invoice.expiry {
            if expiry < Utc::now().timestamp() {
                return Err(PayError::InvoiceExpired);
            }
        }
        let contract_id = invoice.contract.ok_or(PayError::NoContract)?;
        let iface = invoice.iface.ok_or(PayError::NoIface)?;
        let mut main_builder =
            self.transition_builder(contract_id, iface.clone(), invoice.operation.clone())?;

        let (beneficiary_output, beneficiary) = match invoice.beneficiary {
            Beneficiary::BlindedSeal(seal) => {
                let seal = BuilderSeal::Concealed(seal);
                (None, seal)
            }
            Beneficiary::WitnessUtxo(addr) => {
                let vout = psbt
                    .unsigned_tx
                    .output
                    .iter()
                    .enumerate()
                    .find(|(_, txout)| txout.script_pubkey == addr.script_pubkey())
                    .map(|(no, _)| no as u32)
                    .ok_or(PayError::NoBeneficiaryOutput)?;
                let seal = BuilderSeal::Revealed(GraphSeal::new_vout(method, vout));
                (Some(vout), seal)
            }
        };
        let prev_outputs = psbt
            .unsigned_tx
            .input
            .iter()
            .map(|txin| txin.previous_output)
            .map(|outpoint| Outpoint::new(outpoint.txid.to_byte_array().into(), outpoint.vout))
            .collect::<Vec<_>>();

        // Classify PSBT outputs which can be used for assignments
        let mut out_classes = HashMap::<VelocityHint, Vec<u32>>::new();
        for (no, outp) in psbt.outputs.iter().enumerate() {
            if beneficiary_output == Some(no as u32) {
                continue;
            }
            if outp
                // NB: Here we assume that if output has derivation information it belongs to our wallet.
                .bip32_derivation
                .first_key_value()
                .map(|(_, src)| src)
                .or_else(|| {
                    outp.tap_key_origins
                        .first_key_value()
                        .map(|(_, (_, src))| src)
                })
                .and_then(|(_, src)| src.into_iter().rev().nth(1))
                .copied()
                .map(u32::from)
                .is_some()
            {
                let class = outp.rgb_velocity_hint().unwrap_or_default();
                out_classes.entry(class).or_default().push(no as u32);
            }
        }
        let mut out_classes = out_classes
            .into_iter()
            .map(|(class, indexes)| (class, indexes.into_iter().cycle()))
            .collect::<HashMap<_, _>>();
        let mut output_for_assignment = |suppl: Option<&ContractSuppl>,
                                         assignment_type: AssignmentType|
         -> Result<BuilderSeal<GraphSeal>, PayError<_, _>> {
            let velocity = suppl
                .and_then(|suppl| suppl.owned_state.get(&assignment_type))
                .map(|s| s.velocity)
                .unwrap_or_default();
            let vout = out_classes
                .get_mut(&velocity)
                .and_then(iter::Cycle::next)
                .or_else(|| {
                    out_classes
                        .get_mut(&VelocityHint::default())
                        .and_then(iter::Cycle::next)
                })
                .ok_or(PayError::NoBlankOrChange(velocity, assignment_type))?;
            let seal = GraphSeal::new_vout(method, vout);
            Ok(BuilderSeal::Revealed(seal))
        };

        // 2. Prepare and self-consume transition
        let assignment_name = invoice
            .assignment
            .as_ref()
            .or_else(|| main_builder.default_assignment().ok())
            .ok_or(BuilderError::NoDefaultAssignment)?;
        let assignment_id = main_builder
            .assignments_type(assignment_name)
            .ok_or(BuilderError::InvalidStateField(assignment_name.clone()))?;
        // TODO: select supplement basing on the signer trust level
        let suppl = self
            .contract_suppl(contract_id)
            .and_then(|set| set.first())
            .cloned();

        let mut sum_inputs = 0u64;
        let mut type_state = TypedState::Void;
        for (opout, state) in self.state_for_outpoints(contract_id, prev_outputs.iter().copied())? {
            main_builder = main_builder.add_input(opout)?;
            if opout.ty != assignment_id {
                let seal = output_for_assignment(suppl.as_ref(), opout.ty)?;
                main_builder = main_builder.add_raw_state(opout.ty, seal, state)?;
            } else if let TypedState::Amount(value) = state {
                sum_inputs += value;
                type_state = state;
            } else if let TypedState::Data(_) = state {
                sum_inputs += 1;
                type_state = state;
            }
        }

        // Retrieve Previous State Transtitions
        let mut previous_state_value = 0u64;
        let mut previous_states: Vec<PreviousSeal> = vec![];
        for invoice in options.other_invoices {
            match invoice.owned_state {
                TypedState::Amount(value) => {
                    previous_state_value += value;
                }
                TypedState::Data(_) => {
                    previous_state_value += 1;
                }
                _ => {
                    todo!("only TypedState::Amount is currently supported")
                }
            }

            let seal = match invoice.beneficiary {
                Beneficiary::BlindedSeal(seal) => seal,
                Beneficiary::WitnessUtxo(addr) => {
                    let vout = psbt
                        .unsigned_tx
                        .output
                        .iter()
                        .enumerate()
                        .find(|(_, txout)| txout.script_pubkey == addr.script_pubkey())
                        .map(|(no, _)| no as u32)
                        .ok_or(PayError::NoBeneficiaryOutput)?;
                    GraphSeal::new_vout(method, vout).to_concealed_seal()
                }
            };

            let prev_seal = PreviousSeal {
                state: invoice.owned_state,
                seal,
            };
            previous_states.push(prev_seal);
        }

        // Add change
        let amt = match invoice.owned_state {
            TypedState::Amount(amt) => {
                if sum_inputs < amt + previous_state_value {
                    return Err(PayError::InsufficientState);
                }

                match sum_inputs.cmp(&amt) {
                    Ordering::Greater => {
                        let seal = output_for_assignment(suppl.as_ref(), assignment_id)?;
                        let change = amt + previous_state_value;
                        let change = TypedState::Amount(sum_inputs - change);
                        main_builder = main_builder.add_raw_state(assignment_id, seal, change)?;
                        amt
                    }
                    Ordering::Equal => amt,
                    Ordering::Less => return Err(PayError::InsufficientState),
                }
            }
            _ => {
                todo!("only TypedState::Amount is currently supported")
            }
        };

        for PreviousSeal { seal, state } in previous_states.clone() {
            let prev_seal = BuilderSeal::Concealed(seal);
            main_builder = match state {
                TypedState::Amount(value) => main_builder.add_raw_state(
                    assignment_id,
                    prev_seal,
                    TypedState::Amount(value),
                )?,
                TypedState::Data(_) => {
                    main_builder.add_raw_state(assignment_id, prev_seal, state)?
                }
                _ => {
                    todo!("Only TypedState::Amount and TypedState::Data are currently supported")
                }
            };
        }

        // Finish Transition
        let transition = match type_state {
            TypedState::Amount(_) => main_builder
                .add_raw_state(assignment_id, beneficiary, TypedState::Amount(amt))?
                .complete_transition(contract_id)?,
            TypedState::Data(_) => main_builder
                .add_raw_state(assignment_id, beneficiary, type_state)?
                .complete_transition(contract_id)?,
            _ => {
                todo!("Only TypedState::Amount and TypedState::Data are currently supported")
            }
        };

        // 3. Prepare and self-consume other transitions
        let mut contract_inputs = HashMap::<ContractId, Vec<Outpoint>>::new();
        let mut spent_state = HashMap::<ContractId, BTreeMap<Opout, TypedState>>::new();

        for outpoint in prev_outputs {
            for id in self.contracts_by_outpoints([outpoint])? {
                contract_inputs.entry(id).or_default().push(outpoint);
                if id == contract_id {
                    continue;
                }
                spent_state
                    .entry(id)
                    .or_default()
                    .extend(self.state_for_outpoints(id, [outpoint])?);
            }
        }

        // Construct blank transitions, self-consume them
        let mut other_transitions = HashMap::with_capacity(spent_state.len());
        for (id, opouts) in spent_state {
            let mut blank_builder = self.blank_builder(id, iface.clone())?;
            // TODO: select supplement basing on the signer trust level
            let suppl = self.contract_suppl(id).and_then(|set| set.first());

            for (opout, state) in opouts {
                let seal = output_for_assignment(suppl, opout.ty)?;
                blank_builder = blank_builder
                    .add_input(opout)?
                    .add_raw_state(opout.ty, seal, state)?;
            }

            other_transitions.insert(id, blank_builder.complete_transition(contract_id)?);
        }

        // 4. Add transitions to PSBT
        other_transitions.insert(contract_id, transition);
        for (id, transition) in other_transitions.clone() {
            let inputs: &Vec<Outpoint> = contract_inputs.get(&id).unwrap();
            for (input, txin) in psbt.inputs.iter_mut().zip(&psbt.unsigned_tx.input) {
                let prevout = txin.previous_output;
                let outpoint = Outpoint::new(prevout.txid.to_byte_array().into(), prevout.vout);
                if inputs.contains(&outpoint) {
                    input.set_rgb_consumer(id, transition.id())?;
                }
            }
            psbt.push_rgb_transition(transition)?;
        }

        // Here we assume the provided PSBT is final: its inputs and outputs will not be
        // modified after calling this method.
        let bundles = psbt.rgb_bundles()?;
        // TODO: Make it two-staged, such that PSBT editing will be allowed by other
        //       participants as required for multiparty protocols like coinjoin.
        psbt.rgb_bundle_to_lnpbp4()?;
        let anchor = psbt.dbc_conclude(method)?;
        // TODO: Ensure that with PSBTv2 we remove flag allowing PSBT modification.

        // 5. Prepare transfer
        let witness_txid = psbt.unsigned_tx.txid();
        self.consume_anchor(anchor)?;
        for (id, bundle) in bundles {
            self.consume_bundle(id, bundle, witness_txid.to_byte_array().into())?;
        }

        let beneficiary = match beneficiary {
            BuilderSeal::Revealed(seal) => BuilderSeal::Revealed(
                seal.resolve(Txid::from_byte_array(witness_txid.to_byte_array())),
            ),
            BuilderSeal::Concealed(seal) => BuilderSeal::Concealed(seal),
        };

        // 6.Prepare strict transfers
        let mut transfers = vec![];
        if options.strict {
            transfers.push(self.transfer(contract_id, vec![beneficiary])?);
            for prev_seal in previous_states {
                let transfer =
                    self.transfer(contract_id, [BuilderSeal::Concealed(prev_seal.seal)])?;
                transfers.push(transfer);
            }
        } else {
            let transfer = self.transfer(contract_id, vec![beneficiary])?;
            transfers = vec![transfer];
        }

        Ok(transfers)
    }
}

impl<I> ConsignmentEx for I where I: Inventory {}
