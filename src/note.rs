#[derive(Debug)]
pub struct Highlight {
    pub name: String,
    pub highlight_type: HighlightType,
    pub pages: [String; 2],
    pub date_added: String,
    pub highlight: String,
}

#[derive(Debug)]
pub enum HighlightType {
    Highlight,
    Comment,
}

impl std::fmt::Display for Highlight {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let prefix = match self.highlight_type {
        HighlightType::Highlight => "",
        HighlightType::Comment => "NOTE: ",
        };
        write!(f, "{}{}", prefix, self.highlight.replace("\r", ""))
    }
}

impl Highlight {
    pub fn filestem(&self) -> String {
        let bad_chars = vec!['(', ')', ',', ':'];
        let letter_tidier = |letter| {
            if bad_chars.contains(&letter) {
                "".to_string()
            } else if letter == ' ' {
                "-".to_string()
            } else {
                letter.to_lowercase().to_string()
            }
        };
        self.name.to_string().chars().map(letter_tidier).collect()
    }
}
