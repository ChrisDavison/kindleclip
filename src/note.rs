pub type BookNotes<'a> = std::collections::HashMap<String, Vec<Highlight<'a>>>;

#[derive(Debug)]
pub struct Highlight<'a> {
    pub name: &'a str,
    pub highlight_type: HighlightType,
    pub pages: [&'a str; 2],
    pub date_added: &'a str,
    pub highlight: String,
}

#[derive(Debug)]
pub enum HighlightType {
    Highlight,
    Comment,
}

impl<'a> std::fmt::Display for Highlight<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let prefix = match self.highlight_type {
            HighlightType::Highlight => "",
            HighlightType::Comment => "NOTE: ",
        };
        write!(f, "{}{}", prefix, self.highlight.replace('\r', ""))
    }
}

impl<'a> Highlight<'a> {
    pub fn filestem(&self) -> String {
        let bad_chars = ['(', ')', ',', ':'];
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
