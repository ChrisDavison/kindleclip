use anyhow::{Context, Result};

pub fn choose_from_list(ls: &[impl ToString]) -> Result<Vec<String>> {
    for (i, key) in ls.iter().enumerate() {
        println!("{}: {}", i, key.to_string());
    }
    let mut response = String::new();
    std::io::stdin()
        .read_line(&mut response)
        .with_context(|| format!("Failed to get response for which books to export"))?;
    let mut books = Vec::new();
    for choice in response.split(|c| " ,".contains(c)) {
        let choice: usize = choice
            .trim()
            .parse()
            .with_context(|| format!("Failed to parse book choice `{}`", choice))?;
        books.push(ls[choice].to_string());
    }
    Ok(books)
}
