macro_rules! read_compressed {
    ($file: ident, $function: expr $(, $param: expr )* ) => {{
        let filename = $file
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_else(|| panic!("Failed to infer extension for the file {}.", $file.display()));

        let reader = File::open($file).unwrap_or_else(|x| panic!("Failed to open {}: {}.", $file.display(), x));
        let reader = BufReader::new(reader);

        match filename.split('.').last() {
            Some("gz") => {
                let reader = BufReader::new(MultiGzDecoder::new(reader));
                $function(reader $(, $param)*)
            }
            Some(_) | None => $function(reader $(, $param)*),
        }
    }};
}

pub(crate) use read_compressed;
