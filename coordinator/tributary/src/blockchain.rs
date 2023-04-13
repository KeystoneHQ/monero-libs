use std::collections::{HashSet, HashMap};

use ciphersuite::{Ciphersuite, Ristretto};

use crate::{Signed, TransactionKind, Transaction, ProvidedTransactions, BlockError, Block, Mempool};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Blockchain<T: Transaction> {
  genesis: [u8; 32],
  // TODO: db
  block_number: u64,
  tip: [u8; 32],
  next_nonces: HashMap<<Ristretto as Ciphersuite>::G, u32>,

  provided: ProvidedTransactions<T>,
  mempool: Mempool<T>,
}

impl<T: Transaction> Blockchain<T> {
  pub fn new(genesis: [u8; 32], participants: &[<Ristretto as Ciphersuite>::G]) -> Self {
    // TODO: Reload block_number/tip/next_nonces/provided/mempool

    let mut next_nonces = HashMap::new();
    for participant in participants {
      next_nonces.insert(*participant, 0);
    }

    Self {
      genesis,

      block_number: 0,
      tip: genesis,
      next_nonces,

      provided: ProvidedTransactions::new(),
      mempool: Mempool::new(genesis),
    }
  }

  pub fn tip(&self) -> [u8; 32] {
    self.tip
  }

  pub fn block_number(&self) -> u64 {
    self.block_number
  }

  pub fn add_transaction(&mut self, tx: T) -> bool {
    self.mempool.add(&self.next_nonces, tx)
  }

  pub fn provide_transaction(&mut self, tx: T) {
    self.provided.provide(tx)
  }

  /// Returns the next nonce, or None if they aren't a participant.
  pub fn next_nonce(&self, key: <Ristretto as Ciphersuite>::G) -> Option<u32> {
    self.next_nonces.get(&key).cloned()
  }

  pub fn build_block(&mut self) -> Block<T> {
    let block = Block::new(self.tip, &self.provided, self.mempool.block(&self.next_nonces));
    // build_block should not return invalid blocks
    self.verify_block(&block).unwrap();
    block
  }

  pub fn verify_block(&self, block: &Block<T>) -> Result<(), BlockError> {
    let mut locally_provided = HashSet::new();
    for provided in self.provided.transactions.keys() {
      locally_provided.insert(*provided);
    }
    block.verify(self.genesis, self.tip, locally_provided, self.next_nonces.clone())
  }

  /// Add a block.
  pub fn add_block(&mut self, block: &Block<T>) -> Result<(), BlockError> {
    self.verify_block(block)?;

    // None of the following assertions should be reachable since we verified the block
    self.tip = block.hash();
    self.block_number += 1;
    for tx in &block.transactions {
      match tx.kind() {
        TransactionKind::Provided => {
          assert!(
            self.provided.withdraw(tx.hash()),
            "verified block had a provided transaction we didn't have"
          );
        }
        TransactionKind::Unsigned => {}
        TransactionKind::Signed(Signed { signer, nonce, .. }) => {
          let prev = self
            .next_nonces
            .insert(*signer, nonce + 1)
            .expect("block had signed transaction from non-participant");
          if prev != *nonce {
            panic!("verified block had an invalid nonce");
          }
        }
      }
    }

    Ok(())
  }
}
