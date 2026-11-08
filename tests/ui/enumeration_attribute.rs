use base_web_content::FieldValue;

#[derive(FieldValue)]
#[content(rename_all = "lowercase")]
enum Role {
    Admin,
}

fn main() {}
