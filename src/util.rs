use anyhow::{Context, Result};

pub fn choose_from_list(ls: &[impl ToString], filter: Option<String>) -> Result<Vec<String>> {
    let matching: Vec<_> = ls.iter().filter(|key| match filter {
        None => true,
        Some(ref f) => key.to_string().to_lowercase().contains(f),
    }).collect();
    for (i, key) in matching.iter().enumerate() {
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
        books.push(matching[choice].to_string());
    }
    Ok(books)
}
