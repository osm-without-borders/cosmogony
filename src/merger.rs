use anyhow::Result;
use cosmogony::{file_format::OutputFormat, read_zones_from_file, Zone, ZoneIndex};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct CosmogonyMerger {
    id_offset: usize,
}

fn to_json_stream(
    mut writer: impl std::io::Write,
    zones: impl std::iter::Iterator<Item = Zone>,
) -> Result<()> {
    for z in zones {
        serde_json::to_writer(&mut writer, &z)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

impl CosmogonyMerger {
    fn merge_cosmogony(
        &mut self,
        files: &[PathBuf],
        mut writer: impl std::io::Write,
    ) -> Result<()> {
        for f in files {
            self.read_cosmogony(f, &mut writer)?;
        }
        Ok(())
    }

    fn read_cosmogony(&mut self, file: &Path, writer: impl std::io::Write) -> Result<()> {
        let mut max_id = 0;
        let zones = read_zones_from_file(file)?
            .into_iter()
            .filter_map(|z| z.ok())
            .map(|mut z| {
                z.id = self.get_updated_id(z.id);
                max_id = std::cmp::max(max_id, z.id.index);
                z.parent = z.parent.map(|p| self.get_updated_id(p));
                z
            });
        to_json_stream(writer, zones)?;
        // we update the id_offset, for the next file
        self.id_offset = max_id + 1;
        Ok(())
    }

    fn get_updated_id(&self, idx: ZoneIndex) -> ZoneIndex {
        ZoneIndex {
            index: idx.index + self.id_offset,
        }
    }
}

pub fn merge_cosmogony(files: &[PathBuf], output: &Path) -> Result<()> {
    let mut merger = CosmogonyMerger::default();

    let format = OutputFormat::from_filename(output)?;
    let file = std::fs::File::create(output)?;
    let mut stream = std::io::BufWriter::new(file);
    match format {
        OutputFormat::JsonGz | OutputFormat::Json => panic!(
            "cannot write real cosmogonies, only jsonl/jsonl.gz to be able to stream the files"
        ),
        OutputFormat::JsonStream => {
            merger.merge_cosmogony(files, &mut stream)?;
        }
        OutputFormat::JsonStreamGz => {
            let mut e = GzEncoder::new(stream, Compression::default());
            merger.merge_cosmogony(files, &mut e)?;
        }
    };
    Ok(())
}
