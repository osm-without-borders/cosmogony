use failure::Error;

#[derive(PartialEq, Clone)]
pub enum OutputFormat {
    Json,
    JsonGz,
    JsonStream,
    JsonStreamGz,
}

impl OutputFormat {
    fn all_extensions() -> Vec<(String, OutputFormat)> {
        vec![
            (".json".into(), OutputFormat::Json),
            (".jsonl".into(), OutputFormat::JsonStream),
            (".json.gz".into(), OutputFormat::JsonGz),
            (".jsonl.gz".into(), OutputFormat::JsonStreamGz),
        ]
    }

    pub fn from_filename(filename: &str) -> Result<OutputFormat, Error> {
        let extensions = OutputFormat::all_extensions();
        extensions
            .iter()
            .find(|&&(ref e, _)| filename.ends_with(e))
            .map(|&(_, ref f)| f.clone())
            .ok_or_else(|| {
                let extensions_str = extensions
                    .into_iter()
                    .map(|(e, _)| e)
                    .collect::<Vec<String>>()
                    .join(", ");
                failure::err_msg(format!(
                    "Unable to detect the file format from filename '{}'. \
                     Accepted extensions are: {}",
                    filename, extensions_str
                ))
            })
    }
}
