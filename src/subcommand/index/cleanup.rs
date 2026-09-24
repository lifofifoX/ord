use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Cleanup {
  #[arg(long, help = "Delete pre-jubilee filtered inscription shadow data.")]
  pre_jubilee_filtered: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Output {
  pub rows_before: u64,
  pub rows_after: u64,
  pub table_deleted: bool,
}

impl Cleanup {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    if !self.pre_jubilee_filtered {
      bail!("no cleanup target selected, pass --pre-jubilee-filtered");
    }

    let index = Index::open(&settings)?;
    let (rows_before, rows_after, table_deleted) =
      index.cleanup_pre_jubilee_filtered_inscription_data()?;

    Ok(Some(Box::new(Output {
      rows_before,
      rows_after,
      table_deleted,
    })))
  }
}
