//! This module contains the logic for parsing and interacting with EVM
//! execution traces.
pub(crate) mod exec_step;
use crate::evm::EvmWord;
use crate::operation::{container::OperationContainer, Operation};
use crate::operation::{MemoryOp, StackOp, StorageOp, Target};
use crate::Error;
use core::ops::{Index, IndexMut};
pub use exec_step::ExecutionStep;
use pasta_curves::arithmetic::FieldExt;
use std::convert::TryFrom;

use self::exec_step::ParsedExecutionStep;

/// Definition of all of the constants related to an Ethereum block and
/// therefore, related with an [`ExecutionTrace`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockConstants<F: FieldExt> {
    hash: EvmWord, // Until we know how to deal with it
    coinbase: F,
    timestamp: F,
    number: F,
    difficulty: F,
    gas_limit: F,
    chain_id: F,
    base_fee: F,
}

impl<F: FieldExt> BlockConstants<F> {
    #[allow(clippy::too_many_arguments)]
    /// Generates a new `BlockConstants` instance from it's fields.
    pub fn new(
        hash: EvmWord,
        coinbase: F,
        timestamp: F,
        number: F,
        difficulty: F,
        gas_limit: F,
        chain_id: F,
        base_fee: F,
    ) -> BlockConstants<F> {
        BlockConstants {
            hash,
            coinbase,
            timestamp,
            number,
            difficulty,
            gas_limit,
            chain_id,
            base_fee,
        }
    }
    #[inline]
    /// Return the hash of a block.
    pub fn hash(&self) -> &EvmWord {
        &self.hash
    }

    #[inline]
    /// Return the coinbase of a block.
    pub fn coinbase(&self) -> &F {
        &self.coinbase
    }

    #[inline]
    /// Return the timestamp of a block.
    pub fn timestamp(&self) -> &F {
        &self.timestamp
    }

    #[inline]
    /// Return the block number.
    pub fn number(&self) -> &F {
        &self.number
    }

    #[inline]
    /// Return the difficulty of a block.
    pub fn difficulty(&self) -> &F {
        &self.difficulty
    }

    #[inline]
    /// Return the gas_limit of a block.
    pub fn gas_limit(&self) -> &F {
        &self.gas_limit
    }

    #[inline]
    /// Return the chain ID associated to a block.
    pub fn chain_id(&self) -> &F {
        &self.chain_id
    }

    #[inline]
    /// Return the base fee of a block.
    pub fn base_fee(&self) -> &F {
        &self.base_fee
    }
}

/// Result of the parsing of an EVM execution trace.
/// This structure is the centre of the crate and is intended to be the only
/// entry point to it. The `ExecutionTrace` provides three main actions:
///
/// 1. Generate an `ExecutionTrace` instance by parsing an EVM trace (JSON
/// format for now).
///
/// 2. Generate and provide an iterator over all of the
/// [`Instruction`](crate::evm::Instruction)s of the trace and apply it's
/// respective constraints into a provided a mutable reference to a
/// [`ConstraintSystyem`](halo2::plonk::ConstraintSystem).
///
/// 3. Generate and provide and ordered list of all of the
/// [`StackOp`](crate::operation::StackOp)s,
/// [`MemoryOp`](crate::operation::MemoryOp)s and
/// [`StorageOp`](crate::operation::StorageOp)s that each
/// [`Instruction`](crate::evm::Instruction) that derive from the trace so that
/// the State Proof witnesses are already obtained on a structured manner and
/// ready to be added into the State circuit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionTrace<F: FieldExt> {
    steps: Vec<ExecutionStep>,
    block_ctants: BlockConstants<F>,
    container: OperationContainer,
}

impl<F: FieldExt> Index<usize> for ExecutionTrace<F> {
    type Output = ExecutionStep;
    fn index(&self, index: usize) -> &Self::Output {
        &self.steps[index]
    }
}

impl<F: FieldExt> IndexMut<usize> for ExecutionTrace<F> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.steps[index]
    }
}

impl<F: FieldExt> ExecutionTrace<F> {
    /// Given an EVM trace in JSON format according to the specs and format
    /// shown in [zkevm-test-vectors crate](https://github.com/appliedzkp/zkevm-testing-vectors), generate an `ExecutionTrace`
    /// and generate all of the [`Operation`]s associated to each one of it's
    /// [`ExecutionStep`]s filling them bus-mapping instances.
    pub fn from_trace_bytes<T: AsRef<[u8]>>(
        bytes: T,
        block_ctants: BlockConstants<F>,
    ) -> Result<ExecutionTrace<F>, Error> {
        let trace_loaded =
            serde_json::from_slice::<Vec<ParsedExecutionStep>>(bytes.as_ref())
                .map_err(|_| Error::SerdeError)?
                .iter()
                .map(ExecutionStep::try_from)
                .collect::<Result<Vec<ExecutionStep>, Error>>()?;

        Ok(ExecutionTrace::<F>::new(trace_loaded, block_ctants))
    }

    /// Given a vector of [`ExecutionStep`]s and a [`BlockConstants`] instance,
    /// generate an [`ExecutionTrace`] by:
    ///
    /// 1) Setting the correct [`GlobalCounter`](crate::evm::GlobalCounter) to
    /// each [`ExecutionStep`].
    /// 2) Generating the corresponding [`Operation`]s, registering them in the
    /// container and storing the [`OperationRef`]s to each one of the
    /// generated ops into the bus-mapping instances of each [`ExecutionStep`].
    pub(crate) fn new(
        steps: Vec<ExecutionStep>,
        block_ctants: BlockConstants<F>,
    ) -> Self {
        ExecutionTrace {
            steps,
            block_ctants,
            /// Dummy empty container to enable build.
            container: OperationContainer::new(),
        }
        .build()
    }

    /// Returns an ordered `Vec` containing all the [`StackOp`]s of the actual
    /// `ExecutionTrace` so that they can be directly included in the State
    /// proof.
    pub fn sorted_stack_ops(&self) -> Vec<StackOp> {
        self.container.sorted_stack()
    }

    /// Returns an ordered `Vec` containing all the [`MemoryOp`]s of the actual
    /// `ExecutionTrace` so that they can be directly included in the State
    /// proof.
    pub fn sorted_memory_ops(&self) -> Vec<MemoryOp> {
        self.container.sorted_memory()
    }

    /// Returns an ordered `Vec` containing all the [`StorageOp`]s of the actual
    /// `ExecutionTrace` so that they can be directly included in the State
    /// proof.
    pub fn sorted_storage_ops(&self) -> Vec<StorageOp> {
        self.container.sorted_storage()
    }

    /// Traverses the trace step by step, and for each [`ExecutionStep`]:
    /// 1. Sets the correct [`GlobalCounter`](crate::evm::GlobalCounter).
    /// 2. Generates the corresponding [`Operation`]s and stores them inside the
    /// [`OperationContainer`] instance stored inside of the trace + adds the
    /// [`OperationRef`]s obtained from the container addition into each
    /// [`ExecutionStep`] bus-mapping instances.
    fn build(mut self) -> Self {
        // Set a counter to add the correct global counters.
        let mut gc = 0usize;
        let mut new_container = OperationContainer::new();
        self.steps_mut().iter_mut().for_each(|exec_step| {
            // Set correct global counter
            exec_step.set_gc(gc);
            // Add the `OpcodeId` associated ops and increment the gc counting
            // all of them.
            gc += exec_step.gen_associated_ops::<F>(&mut new_container);
            // Sum 1 to counter so that we set the next exec_step GC to the
            // correct index
            gc += 1;
        });

        // Replace the empty original container with the new one we just filled.
        self.container = new_container;
        self
    }

    /// Registers an [`Operation`] into the [`OperationContainer`] and then adds
    /// a reference to the stored operation ([`OperationRef`]) inside the
    /// bus-mapping instance of the [`ExecutionStep`] located at `exec_step_idx`
    /// inside the [`ExecutionTrace`].
    pub(crate) fn add_op_to_container(
        &mut self,
        op: Operation,
        exec_step_idx: usize,
    ) {
        let op_ref = self.container_mut().insert(op);
        self.steps[exec_step_idx]
            .bus_mapping_instance_mut()
            .push(op_ref);
    }

    /// Returns a reference to the [`ExecutionStep`] vector instance
    /// that the `ExecutionTrace` holds.
    pub fn steps(&self) -> &Vec<ExecutionStep> {
        &self.steps
    }

    /// Returns a mutable reference to the [`ExecutionStep`] vector instance
    /// that the `ExecutionTrace` holds.
    fn steps_mut(&mut self) -> &mut Vec<ExecutionStep> {
        &mut self.steps
    }

    /// Returns a mutable reference to the [`OperationContainer`] instance that
    /// the `ExecutionTrace` holds.
    fn container_mut(&mut self) -> &mut OperationContainer {
        &mut self.container
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The target and index of an `Operation` in the context of an
/// `ExecutionTrace`.
pub struct OperationRef(Target, usize);

impl From<(Target, usize)> for OperationRef {
    fn from(op_ref_data: (Target, usize)) -> Self {
        match op_ref_data.0 {
            Target::Memory => Self(Target::Memory, op_ref_data.1),
            Target::Stack => Self(Target::Stack, op_ref_data.1),
            Target::Storage => Self(Target::Storage, op_ref_data.1),
        }
    }
}

impl OperationRef {
    /// Return the `OperationRef` as a `usize`.
    pub const fn as_usize(&self) -> usize {
        self.1
    }

    /// Return the [`Target`] op type of the `OperationRef`.
    pub const fn target(&self) -> Target {
        self.0
    }
}

#[cfg(test)]
mod trace_tests {
    use super::*;
    use crate::{
        evm::{
            opcodes::ids::OpcodeId, GlobalCounter, Instruction, MemoryAddress,
            ProgramCounter, StackAddress,
        },
        exec_trace::ExecutionStep,
        operation::{StackOp, RW},
    };
    use alloc::collections::BTreeMap;
    use num::BigUint;

    #[test]
    fn exec_trace_parsing() {
        let input_trace = r#"
        [
            {
                "memory": {
                    "0": "0000000000000000000000000000000000000000000000000000000000000000",
                    "20": "0000000000000000000000000000000000000000000000000000000000000000",
                    "40": "0000000000000000000000000000000000000000000000000000000000000000"
                },
                "stack": [
                    "40"
                ],
                "opcode": "PUSH1 40",
                "pc": 0
            },
            {
                "memory": {
                    "00": "0000000000000000000000000000000000000000000000000000000000000000",
                    "20": "0000000000000000000000000000000000000000000000000000000000000000",
                    "40": "0000000000000000000000000000000000000000000000000000000000000000"
                },
                "stack": [
                    "40",
                    "80"
                ],
                "opcode": "PUSH1 80",
                "pc": 1
            }
        ]
        "#;

        let block_ctants = BlockConstants::new(
            EvmWord::from(0u8),
            pasta_curves::Fp::zero(),
            pasta_curves::Fp::zero(),
            pasta_curves::Fp::zero(),
            pasta_curves::Fp::zero(),
            pasta_curves::Fp::zero(),
            pasta_curves::Fp::zero(),
            pasta_curves::Fp::zero(),
        );

        // Generate the expected ExecutionTrace corresponding to the JSON
        // provided above.

        // Container is shared across ExecutionSteps
        let mut container = OperationContainer::new();

        // The memory is the same in both steps as none of them touches the
        // memory of the EVM.
        let mut mem_map = BTreeMap::new();
        mem_map
            .insert(MemoryAddress(BigUint::from(0x00u8)), EvmWord::from(0u8));
        mem_map
            .insert(MemoryAddress(BigUint::from(0x20u8)), EvmWord::from(0u8));
        mem_map
            .insert(MemoryAddress(BigUint::from(0x40u8)), EvmWord::from(0u8));

        // Generate Step1 corresponding to PUSH1 40
        let mut step_1 = ExecutionStep::new(
            mem_map.clone(),
            vec![EvmWord::from(0x40u8)],
            Instruction::new(OpcodeId::PUSH1, Some(EvmWord::from(0x40u8))),
            ProgramCounter::from(0),
            GlobalCounter::from(0),
        );

        // Add StackOp associated to this opcode to the container &
        // step.bus_mapping
        step_1
            .bus_mapping_instance_mut()
            .push(container.insert(StackOp::new(
                RW::WRITE,
                GlobalCounter(1usize),
                StackAddress::from(1023),
                EvmWord::from(0x40u8),
            )));

        // Generate Step2 corresponding to PUSH1 80
        let mut step_2 = ExecutionStep::new(
            mem_map,
            vec![EvmWord::from(0x40u8), EvmWord::from(0x80u8)],
            Instruction::new(OpcodeId::PUSH1, Some(EvmWord::from(0x80u8))),
            ProgramCounter::from(1),
            GlobalCounter::from(2),
        );

        // Add StackOp associated to this opcode to the container &
        // step.bus_mapping
        step_2
            .bus_mapping_instance_mut()
            .push(container.insert(StackOp::new(
                RW::WRITE,
                GlobalCounter(3usize),
                StackAddress::from(1022),
                EvmWord::from(0x80u8),
            )));
        let expected_exec_trace = ExecutionTrace {
            steps: vec![step_1, step_2],
            block_ctants: block_ctants.clone(),
            container,
        };

        // Obtained trace computation
        let obtained_exec_trace = ExecutionTrace::from_trace_bytes(
            input_trace.as_bytes(),
            block_ctants,
        )
        .expect("Error on trace generation");

        assert_eq!(obtained_exec_trace, expected_exec_trace)
    }
}
