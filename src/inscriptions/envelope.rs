use {
  super::*,
  bitcoin::blockdata::{
    opcodes,
    script::{
      Instruction::{self, Op, PushBytes},
      Instructions,
    },
  },
  std::iter::Peekable,
};

pub(crate) const PROTOCOL_ID: [u8; 3] = *b"ord";
pub(crate) const BODY_TAG: [u8; 0] = [];
pub(crate) const BIP110_MAX_PUSH_SIZE: usize = 256;

type Result<T> = std::result::Result<T, script::Error>;
pub type RawEnvelope = Envelope<Vec<Vec<u8>>>;
pub type ParsedEnvelope = Envelope<Inscription>;

fn pushnum_value(opcode: opcodes::Opcode) -> Option<u8> {
  match opcode {
    opcodes::all::OP_PUSHNUM_NEG1 => Some(0x81),
    opcodes::all::OP_PUSHNUM_1 => Some(1),
    opcodes::all::OP_PUSHNUM_2 => Some(2),
    opcodes::all::OP_PUSHNUM_3 => Some(3),
    opcodes::all::OP_PUSHNUM_4 => Some(4),
    opcodes::all::OP_PUSHNUM_5 => Some(5),
    opcodes::all::OP_PUSHNUM_6 => Some(6),
    opcodes::all::OP_PUSHNUM_7 => Some(7),
    opcodes::all::OP_PUSHNUM_8 => Some(8),
    opcodes::all::OP_PUSHNUM_9 => Some(9),
    opcodes::all::OP_PUSHNUM_10 => Some(10),
    opcodes::all::OP_PUSHNUM_11 => Some(11),
    opcodes::all::OP_PUSHNUM_12 => Some(12),
    opcodes::all::OP_PUSHNUM_13 => Some(13),
    opcodes::all::OP_PUSHNUM_14 => Some(14),
    opcodes::all::OP_PUSHNUM_15 => Some(15),
    opcodes::all::OP_PUSHNUM_16 => Some(16),
    _ => None,
  }
}

#[derive(Default, PartialEq, Clone, Serialize, Deserialize, Debug, Eq)]
pub struct Envelope<T> {
  pub input: u32,
  pub offset: u32,
  pub payload: T,
  pub pushnum: bool,
  pub stutter: bool,
}

impl From<RawEnvelope> for ParsedEnvelope {
  fn from(envelope: RawEnvelope) -> Self {
    let body = envelope
      .payload
      .iter()
      .enumerate()
      .position(|(i, push)| i % 2 == 0 && push.is_empty());

    let mut fields: BTreeMap<&[u8], Vec<&[u8]>> = BTreeMap::new();

    let mut incomplete_field = false;

    for item in envelope.payload[..body.unwrap_or(envelope.payload.len())].chunks(2) {
      match item {
        [key, value] => fields.entry(key).or_default().push(value),
        _ => incomplete_field = true,
      }
    }

    let duplicate_field = fields.iter().any(|(_key, values)| values.len() > 1);

    let content_encoding = Tag::ContentEncoding.take(&mut fields);
    let content_type = Tag::ContentType.take(&mut fields);
    let delegate = Tag::Delegate.take(&mut fields);
    let metadata = Tag::Metadata.take(&mut fields);
    let metaprotocol = Tag::Metaprotocol.take(&mut fields);
    let parents = Tag::Parent.take_array(&mut fields);
    let pointer = Tag::Pointer.take(&mut fields);
    let properties = Tag::Properties.take(&mut fields);
    let property_encoding = Tag::PropertyEncoding.take(&mut fields);
    let rune = Tag::Rune.take(&mut fields);

    let unrecognized_even_field = fields
      .keys()
      .any(|tag| tag.first().map(|lsb| lsb % 2 == 0).unwrap_or_default());

    Self {
      payload: Inscription {
        body: body.map(|i| {
          envelope.payload[i + 1..]
            .iter()
            .flatten()
            .cloned()
            .collect()
        }),
        content_encoding,
        content_type,
        delegate,
        duplicate_field,
        incomplete_field,
        metadata,
        metaprotocol,
        parents,
        pointer,
        properties,
        property_encoding,
        rune,
        unrecognized_even_field,
      },
      input: envelope.input,
      offset: envelope.offset,
      pushnum: envelope.pushnum,
      stutter: envelope.stutter,
    }
  }
}

impl ParsedEnvelope {
  pub fn from_transaction(transaction: &Transaction) -> Vec<Self> {
    RawEnvelope::from_transaction(transaction)
      .into_iter()
      .map(|envelope| envelope.into())
      .collect()
  }
}

impl RawEnvelope {
  pub fn from_transaction(transaction: &Transaction) -> Vec<Self> {
    let mut envelopes = Vec::new();

    for (i, input) in transaction.input.iter().enumerate() {
      if let Some(tapscript) = unversioned_leaf_script_from_witness(&input.witness)
        && let Ok(input_envelopes) = Self::from_tapscript(tapscript, i)
      {
        envelopes.extend(input_envelopes);
      }
    }

    envelopes
  }

  fn from_tapscript(tapscript: &Script, input: usize) -> Result<Vec<Self>> {
    let mut envelopes = Vec::new();

    let mut instructions = tapscript.instructions().peekable();

    let mut stuttered = false;
    while let Some(instruction) = instructions.next().transpose()? {
      if instruction == PushBytes((&[]).into()) {
        let (stutter, envelope) =
          Self::from_instructions(&mut instructions, input, envelopes.len(), stuttered)?;
        if let Some(envelope) = envelope {
          envelopes.push(envelope);
        } else {
          stuttered = stutter;
        }
      } else if instruction == PushBytes((&PROTOCOL_ID).into())
        && let Some(envelope) =
          Self::from_bip110_instructions(&mut instructions, input, envelopes.len())?
      {
        envelopes.push(envelope);
      }
    }

    Ok(envelopes)
  }

  fn accept(instructions: &mut Peekable<Instructions>, instruction: Instruction) -> Result<bool> {
    if instructions.peek() == Some(&Ok(instruction)) {
      instructions.next().transpose()?;
      Ok(true)
    } else {
      Ok(false)
    }
  }

  fn from_instructions(
    instructions: &mut Peekable<Instructions>,
    input: usize,
    offset: usize,
    stutter: bool,
  ) -> Result<(bool, Option<Self>)> {
    if !Self::accept(instructions, Op(opcodes::all::OP_IF))? {
      let stutter = instructions.peek() == Some(&Ok(PushBytes((&[]).into())));
      return Ok((stutter, None));
    }

    if !Self::accept(instructions, PushBytes((&PROTOCOL_ID).into()))? {
      let stutter = instructions.peek() == Some(&Ok(PushBytes((&[]).into())));
      return Ok((stutter, None));
    }

    let mut pushnum = false;

    let mut payload = Vec::new();

    loop {
      match instructions.next().transpose()? {
        None => return Ok((false, None)),
        Some(Op(opcodes::all::OP_ENDIF)) => {
          return Ok((
            false,
            Some(Envelope {
              input: input.try_into().unwrap(),
              offset: offset.try_into().unwrap(),
              payload,
              pushnum,
              stutter,
            }),
          ));
        }
        Some(Op(op)) => {
          let Some(value) = pushnum_value(op) else {
            return Ok((false, None));
          };
          pushnum = true;
          payload.push(vec![value]);
        }
        Some(PushBytes(push)) => {
          payload.push(push.as_bytes().to_vec());
        }
      }
    }
  }

  // BIP-110 forbids executing OP_IF and OP_NOTIF in tapscript and limits data
  // pushes to 256 bytes, so a compatible envelope is the protocol push
  // followed by data pushes balanced by OP_DROP and OP_2DROP. The envelope is
  // complete when every element it pushed, including the protocol push, has
  // been dropped.
  fn from_bip110_instructions(
    instructions: &mut Peekable<Instructions>,
    input: usize,
    offset: usize,
  ) -> Result<Option<Self>> {
    let mut payload = Vec::new();

    let mut pushnum = false;

    let mut depth: usize = 1;

    loop {
      match instructions.next().transpose()? {
        None => return Ok(None),
        Some(PushBytes(push)) => {
          if push.len() > BIP110_MAX_PUSH_SIZE {
            return Ok(None);
          }
          payload.push(push.as_bytes().to_vec());
          depth += 1;
        }
        Some(Op(opcodes::all::OP_2DROP)) => {
          if depth < 2 {
            return Ok(None);
          }
          depth -= 2;
        }
        Some(Op(opcodes::all::OP_DROP)) => {
          depth -= 1;
        }
        Some(Op(op)) => {
          let Some(value) = pushnum_value(op) else {
            return Ok(None);
          };
          pushnum = true;
          payload.push(vec![value]);
          depth += 1;
        }
      }

      if depth == 0 {
        return Ok(Some(Envelope {
          input: input.try_into().unwrap(),
          offset: offset.try_into().unwrap(),
          payload,
          pushnum,
          stutter: false,
        }));
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(witnesses: &[Witness]) -> Vec<ParsedEnvelope> {
    ParsedEnvelope::from_transaction(&Transaction {
      version: Version(2),
      lock_time: LockTime::ZERO,
      input: witnesses
        .iter()
        .map(|witness| TxIn {
          previous_output: OutPoint::null(),
          script_sig: ScriptBuf::new(),
          sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
          witness: witness.clone(),
        })
        .collect(),
      output: Vec::new(),
    })
  }

  #[test]
  fn empty() {
    assert_eq!(parse(&[Witness::new()]), Vec::new())
  }

  #[test]
  fn ignore_key_path_spends() {
    assert_eq!(
      parse(&[Witness::from_slice(&[script::Builder::new()
        .push_opcode(opcodes::OP_FALSE)
        .push_opcode(opcodes::all::OP_IF)
        .push_slice(PROTOCOL_ID)
        .push_opcode(opcodes::all::OP_ENDIF)
        .into_script()
        .into_bytes()])]),
      Vec::new()
    );
  }

  #[test]
  fn ignore_key_path_spends_with_annex() {
    assert_eq!(
      parse(&[Witness::from_slice(&[
        script::Builder::new()
          .push_opcode(opcodes::OP_FALSE)
          .push_opcode(opcodes::all::OP_IF)
          .push_slice(PROTOCOL_ID)
          .push_opcode(opcodes::all::OP_ENDIF)
          .into_script()
          .into_bytes(),
        vec![0x50]
      ])]),
      Vec::new()
    );
  }

  #[test]
  fn parse_from_tapscript() {
    assert_eq!(
      parse(&[Witness::from_slice(&[
        script::Builder::new()
          .push_opcode(opcodes::OP_FALSE)
          .push_opcode(opcodes::all::OP_IF)
          .push_slice(PROTOCOL_ID)
          .push_opcode(opcodes::all::OP_ENDIF)
          .into_script()
          .into_bytes(),
        Vec::new()
      ])]),
      vec![ParsedEnvelope { ..default() }]
    );
  }

  #[test]
  fn ignore_unparsable_scripts() {
    let mut script_bytes = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script()
      .into_bytes();
    script_bytes.push(0x01);

    assert_eq!(
      parse(&[Witness::from_slice(&[script_bytes, Vec::new()])]),
      Vec::new()
    );
  }

  #[test]
  fn no_inscription() {
    assert_eq!(
      parse(&[Witness::from_slice(&[
        ScriptBuf::new().into_bytes(),
        Vec::new()
      ])]),
      Vec::new()
    );
  }

  #[test]
  fn duplicate_field() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        Tag::Nop.bytes().as_slice(),
        &[],
        &Tag::Nop.bytes(),
        &[]
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          duplicate_field: true,
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn with_content_type() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[],
        b"ord",
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }]
    );
  }

  #[test]
  fn with_content_encoding() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[9],
        BROTLI.as_bytes(),
        &[],
        b"ord",
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          content_encoding: Some(BROTLI.into()),
          ..inscription("text/plain;charset=utf-8", "ord")
        },
        ..default()
      }]
    );
  }

  #[test]
  fn with_unknown_tag() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        Tag::Nop.bytes().as_slice(),
        b"bar",
        &[],
        b"ord",
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }]
    );
  }

  #[test]
  fn no_body() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8"
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          content_type: Some(b"text/plain;charset=utf-8".to_vec()),
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn no_content_type() {
    assert_eq!(
      parse(&[envelope(&[b"ord", &[], b"foo"])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          body: Some(b"foo".to_vec()),
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn valid_body_in_multiple_pushes() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[],
        b"foo",
        b"bar"
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "foobar"),
        ..default()
      }],
    );
  }

  #[test]
  fn valid_body_in_zero_pushes() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[]
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", ""),
        ..default()
      }]
    );
  }

  #[test]
  fn valid_body_in_multiple_empty_pushes() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", ""),
        ..default()
      }],
    );
  }

  #[test]
  fn valid_ignore_trailing() {
    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"ord")
      .push_opcode(opcodes::all::OP_ENDIF)
      .push_opcode(opcodes::all::OP_CHECKSIG)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }],
    );
  }

  #[test]
  fn valid_ignore_preceding() {
    let script = script::Builder::new()
      .push_opcode(opcodes::all::OP_CHECKSIG)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"ord")
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }],
    );
  }

  #[test]
  fn multiple_inscriptions_in_a_single_witness() {
    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_ENDIF)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"bar")
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![
        ParsedEnvelope {
          payload: inscription("text/plain;charset=utf-8", "foo"),
          ..default()
        },
        ParsedEnvelope {
          payload: inscription("text/plain;charset=utf-8", "bar"),
          offset: 1,
          ..default()
        },
      ],
    );
  }

  #[test]
  fn invalid_utf8_does_not_render_inscription_invalid() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[],
        &[0b10000000]
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", [0b10000000]),
        ..default()
      },],
    );
  }

  #[test]
  fn no_endif() {
    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      Vec::new(),
    );
  }

  #[test]
  fn no_op_false() {
    let script = script::Builder::new()
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      Vec::new(),
    );
  }

  #[test]
  fn empty_envelope() {
    assert_eq!(parse(&[envelope(&[])]), Vec::new());
  }

  #[test]
  fn wrong_protocol_identifier() {
    assert_eq!(parse(&[envelope(&[b"foo"])]), Vec::new());
  }

  #[test]
  fn extract_from_transaction() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[],
        b"ord"
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }],
    );
  }

  #[test]
  fn extract_from_second_input() {
    assert_eq!(
      parse(&[Witness::new(), inscription("foo", [1; 1040]).to_witness()]),
      vec![ParsedEnvelope {
        payload: inscription("foo", [1; 1040]),
        input: 1,
        ..default()
      }]
    );
  }

  #[test]
  fn extract_from_second_envelope() {
    let mut builder = script::Builder::new();
    builder = inscription("foo", [1; 100]).append_reveal_script_to_builder(builder);
    builder = inscription("bar", [1; 100]).append_reveal_script_to_builder(builder);

    assert_eq!(
      parse(&[Witness::from_slice(&[
        builder.into_script().into_bytes(),
        Vec::new()
      ])]),
      vec![
        ParsedEnvelope {
          payload: inscription("foo", [1; 100]),
          ..default()
        },
        ParsedEnvelope {
          payload: inscription("bar", [1; 100]),
          offset: 1,
          ..default()
        }
      ]
    );
  }

  #[test]
  fn inscribe_png() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::ContentType.bytes(),
        b"image/png",
        &[],
        &[1; 100]
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("image/png", [1; 100]),
        ..default()
      }]
    );
  }

  #[test]
  fn chunked_data_is_parsable() {
    let mut witness = Witness::new();

    witness.push(inscription("foo", [1; 1040]).append_reveal_script(script::Builder::new()));

    witness.push([]);

    assert_eq!(
      parse(&[witness]),
      vec![ParsedEnvelope {
        payload: inscription("foo", [1; 1040]),
        ..default()
      }]
    );
  }

  #[test]
  fn round_trip_with_no_fields() {
    let mut witness = Witness::new();

    witness.push(Inscription::default().append_reveal_script(script::Builder::new()));

    witness.push([]);

    assert_eq!(
      parse(&[witness]),
      vec![ParsedEnvelope {
        payload: Inscription::default(),
        ..default()
      }],
    );
  }

  #[test]
  fn unknown_odd_fields_are_ignored() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &Tag::Nop.bytes(), &[0]])]),
      vec![ParsedEnvelope {
        payload: Inscription::default(),
        ..default()
      }],
    );
  }

  #[test]
  fn unknown_even_fields() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &[22], &[0]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          unrecognized_even_field: true,
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn pointer_field_is_recognized() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &[2], &[1]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          pointer: Some(vec![1]),
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn duplicate_pointer_field_makes_inscription_unbound() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &[2], &[1], &[2], &[0]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          pointer: Some(vec![1]),
          duplicate_field: true,
          unrecognized_even_field: true,
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn tag_66_makes_inscriptions_unbound() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &Tag::Unbound.bytes(), &[1]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          unrecognized_even_field: true,
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn incomplete_field() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &[99]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          incomplete_field: true,
          ..default()
        },
        ..default()
      }],
    );
  }

  #[test]
  fn metadata_is_parsed_correctly() {
    assert_eq!(
      parse(&[envelope(&[&PROTOCOL_ID, &Tag::Metadata.bytes(), &[]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          metadata: Some(Vec::new()),
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn metadata_is_parsed_correctly_from_chunks() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::Metadata.bytes(),
        &[0],
        &Tag::Metadata.bytes(),
        &[1]
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          metadata: Some(vec![0, 1]),
          duplicate_field: true,
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn properties_are_parsed_correctly() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::Properties.bytes(),
        &[1, 2, 3]
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          properties: Some(vec![1, 2, 3]),
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn properties_are_parsed_correctly_from_chunks() {
    assert_eq!(
      parse(&[envelope(&[
        &PROTOCOL_ID,
        &Tag::Properties.bytes(),
        &[0],
        &Tag::Properties.bytes(),
        &[1]
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          properties: Some(vec![0, 1]),
          duplicate_field: true,
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn pushnum_opcodes_are_parsed_correctly() {
    const PUSHNUMS: &[(opcodes::Opcode, u8)] = &[
      (opcodes::all::OP_PUSHNUM_NEG1, 0x81),
      (opcodes::all::OP_PUSHNUM_1, 1),
      (opcodes::all::OP_PUSHNUM_2, 2),
      (opcodes::all::OP_PUSHNUM_3, 3),
      (opcodes::all::OP_PUSHNUM_4, 4),
      (opcodes::all::OP_PUSHNUM_5, 5),
      (opcodes::all::OP_PUSHNUM_6, 6),
      (opcodes::all::OP_PUSHNUM_7, 7),
      (opcodes::all::OP_PUSHNUM_8, 8),
      (opcodes::all::OP_PUSHNUM_9, 9),
      (opcodes::all::OP_PUSHNUM_10, 10),
      (opcodes::all::OP_PUSHNUM_11, 11),
      (opcodes::all::OP_PUSHNUM_12, 12),
      (opcodes::all::OP_PUSHNUM_13, 13),
      (opcodes::all::OP_PUSHNUM_14, 14),
      (opcodes::all::OP_PUSHNUM_15, 15),
      (opcodes::all::OP_PUSHNUM_16, 16),
    ];

    for &(op, value) in PUSHNUMS {
      let script = script::Builder::new()
        .push_opcode(opcodes::OP_FALSE)
        .push_opcode(opcodes::all::OP_IF)
        .push_slice(PROTOCOL_ID)
        .push_opcode(opcodes::OP_FALSE)
        .push_opcode(op)
        .push_opcode(opcodes::all::OP_ENDIF)
        .into_script();

      assert_eq!(
        parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
        vec![ParsedEnvelope {
          payload: Inscription {
            body: Some(vec![value]),
            ..default()
          },
          pushnum: true,
          ..default()
        }],
      );
    }
  }

  #[test]
  fn stuttering() {
    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: Default::default(),
        stutter: true,
        ..default()
      }],
    );

    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: Default::default(),
        stutter: true,
        ..default()
      }],
    );

    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: Default::default(),
        stutter: true,
        ..default()
      }],
    );

    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_AND)
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_ENDIF)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: Default::default(),
        stutter: false,
        ..default()
      }],
    );
  }

  fn bip110_envelope(payload: &[&[u8]]) -> Witness {
    let mut builder = script::Builder::new().push_slice(PROTOCOL_ID);

    for data in payload {
      let mut buf = script::PushBytesBuf::new();
      buf.extend_from_slice(data).unwrap();
      builder = builder.push_slice(buf);
    }

    let mut remaining = payload.len() + 1;

    while remaining >= 2 {
      builder = builder.push_opcode(opcodes::all::OP_2DROP);
      remaining -= 2;
    }

    if remaining == 1 {
      builder = builder.push_opcode(opcodes::all::OP_DROP);
    }

    Witness::from_slice(&[builder.into_script().into_bytes(), Vec::new()])
  }

  #[test]
  fn bip110_envelope_with_content_type_and_body() {
    assert_eq!(
      parse(&[bip110_envelope(&[
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8",
        &[],
        b"ord",
      ])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }]
    );
  }

  #[test]
  fn bip110_empty_envelope() {
    assert_eq!(
      parse(&[bip110_envelope(&[])]),
      vec![ParsedEnvelope { ..default() }]
    );
  }

  #[test]
  fn bip110_envelope_with_body_only() {
    assert_eq!(
      parse(&[bip110_envelope(&[&[], b"foo"])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          body: Some(b"foo".to_vec()),
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn bip110_envelope_with_no_body() {
    assert_eq!(
      parse(&[bip110_envelope(&[
        &Tag::ContentType.bytes(),
        b"text/plain;charset=utf-8"
      ])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          content_type: Some(b"text/plain;charset=utf-8".to_vec()),
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn bip110_envelope_with_interleaved_drops() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_slice([])
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_slice(b"bar")
      .push_opcode(opcodes::all::OP_2DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "foobar"),
        ..default()
      }]
    );
  }

  #[test]
  fn bip110_envelope_with_op_3dup_is_invalid() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_3DUP)
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_2DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      Vec::new()
    );
  }

  #[test]
  fn bip110_push_of_256_bytes_is_valid() {
    assert_eq!(
      parse(&[bip110_envelope(&[&[], &[1; 256]])]),
      vec![ParsedEnvelope {
        payload: Inscription {
          body: Some(vec![1; 256]),
          ..default()
        },
        ..default()
      }]
    );
  }

  #[test]
  fn bip110_push_over_256_bytes_is_invalid() {
    assert_eq!(parse(&[bip110_envelope(&[&[], &[1; 257]])]), Vec::new());
  }

  #[test]
  fn bip110_op_2drop_underflowing_envelope_stack_is_invalid() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_2DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      Vec::new()
    );
  }

  #[test]
  fn bip110_envelope_without_balancing_drops_is_invalid() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_slice(b"foo")
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      Vec::new()
    );
  }

  #[test]
  fn bip110_envelope_with_unexpected_opcode_is_invalid() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_CHECKSIG)
      .push_opcode(opcodes::all::OP_2DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      Vec::new()
    );
  }

  #[test]
  fn bip110_envelope_with_surrounding_script() {
    let script = script::Builder::new()
      .push_slice([2; 32])
      .push_opcode(opcodes::all::OP_CHECKSIG)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"ord")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "ord"),
        ..default()
      }]
    );
  }

  #[test]
  fn multiple_bip110_envelopes_in_a_single_witness() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_slice([])
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_DROP)
      .push_slice(PROTOCOL_ID)
      .push_slice([])
      .push_slice(b"bar")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![
        ParsedEnvelope {
          payload: Inscription {
            body: Some(b"foo".to_vec()),
            ..default()
          },
          ..default()
        },
        ParsedEnvelope {
          payload: Inscription {
            body: Some(b"bar".to_vec()),
            ..default()
          },
          offset: 1,
          ..default()
        }
      ]
    );
  }

  #[test]
  fn bip110_envelope_with_pushnum_opcodes() {
    let script = script::Builder::new()
      .push_slice(PROTOCOL_ID)
      .push_opcode(opcodes::all::OP_PUSHNUM_1)
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![ParsedEnvelope {
        payload: inscription("text/plain;charset=utf-8", "foo"),
        pushnum: true,
        ..default()
      }]
    );
  }

  #[test]
  fn bip110_and_legacy_envelopes_in_a_single_witness() {
    let script = script::Builder::new()
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"foo")
      .push_opcode(opcodes::all::OP_ENDIF)
      .push_slice(PROTOCOL_ID)
      .push_slice([1])
      .push_slice(b"text/plain;charset=utf-8")
      .push_slice([])
      .push_slice(b"bar")
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_2DROP)
      .push_opcode(opcodes::all::OP_DROP)
      .into_script();

    assert_eq!(
      parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
      vec![
        ParsedEnvelope {
          payload: inscription("text/plain;charset=utf-8", "foo"),
          ..default()
        },
        ParsedEnvelope {
          payload: inscription("text/plain;charset=utf-8", "bar"),
          offset: 1,
          ..default()
        }
      ]
    );
  }
}
