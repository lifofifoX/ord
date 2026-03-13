use super::{
  entry::{Entry, SatPointValue, TxidValue},
  *,
};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct InscriptionTransferEvent {
  pub block_height: u32,
  pub sequence_number: u32,
  pub from_script_pubkey: Vec<u8>,
  pub to_script_pubkey: Option<Vec<u8>>,
  pub old_satpoint: Option<SatPoint>,
  pub new_satpoint: SatPoint,
  pub spent_as_fee_in_txid: Option<Txid>,
}

pub(crate) type InscriptionTransferEventValue = (
  u32,                   // block height
  u32,                   // sequence number
  Vec<u8>,               // from script pubkey
  Option<Vec<u8>>,       // to script pubkey
  Option<SatPointValue>, // old satpoint
  SatPointValue,         // new satpoint
  Option<TxidValue>,     // spent as fee in txid
);

impl Entry for InscriptionTransferEvent {
  type Value = InscriptionTransferEventValue;

  fn load(
    (
      block_height,
      sequence_number,
      from_script_pubkey,
      to_script_pubkey,
      old_satpoint,
      new_satpoint,
      spent_as_fee_in_txid,
    ): InscriptionTransferEventValue,
  ) -> Self {
    Self {
      block_height,
      sequence_number,
      from_script_pubkey,
      to_script_pubkey,
      old_satpoint: old_satpoint.map(SatPoint::load),
      new_satpoint: SatPoint::load(new_satpoint),
      spent_as_fee_in_txid: spent_as_fee_in_txid.map(Txid::load),
    }
  }

  fn store(self) -> Self::Value {
    (
      self.block_height,
      self.sequence_number,
      self.from_script_pubkey,
      self.to_script_pubkey,
      self.old_satpoint.map(SatPoint::store),
      self.new_satpoint.store(),
      self.spent_as_fee_in_txid.map(Txid::store),
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
      old_satpoint: Some(SatPoint {
        outpoint: OutPoint {
          txid: Txid::all_zeros(),
          vout: 1,
        },
        offset: 7,
      }),
      new_satpoint: SatPoint {
        outpoint: OutPoint {
          txid: Txid::from_byte_array([1; 32]),
          vout: 2,
        },
        offset: 8,
      },
      spent_as_fee_in_txid: Some(Txid::from_byte_array([2; 32])),
    };

    let value = (
      5,
      6,
      vec![1, 2, 3],
      Some(vec![4, 5, 6]),
      Some(
        SatPoint {
          outpoint: OutPoint {
            txid: Txid::all_zeros(),
            vout: 1,
          },
          offset: 7,
        }
        .store(),
      ),
      SatPoint {
        outpoint: OutPoint {
          txid: Txid::from_byte_array([1; 32]),
          vout: 2,
        },
        offset: 8,
      }
      .store(),
      Some(Txid::from_byte_array([2; 32]).store()),
    );

    assert_eq!(event.clone().store(), value);
    assert_eq!(InscriptionTransferEvent::load(value), event);
  }
}
