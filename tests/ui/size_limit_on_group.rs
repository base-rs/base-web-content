use base_web_content::Content;

#[derive(Content)]
struct Address {
    city: String,
}

#[derive(Content)]
struct Person {
    #[content(size_limit = 100)]
    address: Address,
}

fn main() {}
