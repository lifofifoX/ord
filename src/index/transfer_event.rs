use super::entry::Entry;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct InscriptionTransferEvent {
  pub block_height: u32,
  pub sequence_number: u32,
  pub from_script_pubkey: Vec<u8>,
  pub to_script_pubkey: Option<Vec<u8>>,
}

pub(crate) type InscriptionTransferEventValue = (
  u32,             // block height
  u32,             // sequence number
  Vec<u8>,         // from script pubkey
  Option<Vec<u8>>, // to script pubkey
);

impl Entry for InscriptionTransferEvent {
  type Value = InscriptionTransferEventValue;

  fn load(
    (block_height, sequence_number, from_script_pubkey, to_script_pubkey): InscriptionTransferEventValue,
  ) -> Self {
    Self {
      block_height,
      sequence_number,
      from_script_pubkey,
      to_script_pubkey,
    }
  }

  fn store(self) -> Self::Value {
    (
      self.block_height,
      self.sequence_number,
      self.from_script_pubkey,
      self.to_script_pubkey,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn inscription_transfer_event_entry() {
    let event = InscriptionTransferEvent {
      block_height: 5,
      sequence_number: 6,
      from_script_pubkey: vec![1, 2, 3],
      to_script_pubkey: Some(vec![4, 5, 6]),
    };

    let value = (5, 6, vec![1, 2, 3], Some(vec![4, 5, 6]));

    assert_eq!(event.clone().store(), value);
    assert_eq!(InscriptionTransferEvent::load(value), event);
  }
}
