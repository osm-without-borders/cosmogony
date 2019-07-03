use failure::Error;
use std::path::Path;

#[derive(PartialEq, Clone)]
pub enum OutputFormat {
    Json,
    JsonGz,
    JsonStream,
    JsonStreamGz,
}

static ALL_EXTENTIONS: [(&str, OutputFormat); 4] = [
    (".json", OutputFormat::Json),
    (".jsonl", OutputFormat::JsonStream),
    (".json.gz", OutputFormat::JsonGz),
    (".jsonl.gz", OutputFormat::JsonStreamGz),
];

impl OutputFormat {
    pub fn from_filename(filename: impl AsRef<Path>) -> Result<OutputFormat, Error> {
        ALL_EXTENTIONS
            .iter()
            .find(|&&(ref e, _)| {
                filename
                    .as_ref()
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map_or(false, |f| f.ends_with(e))
            })
            .map(|&(_, ref f)| f.clone())
            .ok_or_else(|| {
                let extensions_str = ALL_EXTENTIONS
                    .into_iter()
                    .map(|(e, _)| *e)
                    .collect::<Vec<_>>()
                    .join(", ");
                failure::err_msg(format!(
                    "Unable to detect the file format from filename '{}'. \
                     Accepted extensions are: {}",
                    filename.as_ref().display(),
                    extensions_str
                ))
            })
    }
}
