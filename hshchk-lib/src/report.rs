pub struct Report {
    file_name: String,
}

impl Report {
    pub fn new(file_name: String) -> Report {
        Report {
            file_name: file_name,
        }
    }
}