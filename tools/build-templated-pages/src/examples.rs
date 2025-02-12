use core::cmp::Ordering;
use std::fs::File;

use hashbrown::HashMap;
use serde::Serialize;
use tera::{Context, Tera};
use toml_edit::{DocumentMut, Item};

use crate::Command;

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Category {
    description: Option<String>,
    examples: Vec<Example>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Example {
    technical_name: String,
    path: String,
    name: String,
    description: String,
    category: String,
    wasm: bool,
}

impl Ord for Example {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.category, &self.name).cmp(&(&other.category, &other.name))
    }
}

impl PartialOrd for Example {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_examples(panic_on_missing: bool) -> Vec<Example> {
    let manifest_file = std::fs::read_to_string("Cargo.toml").unwrap();
    let manifest = manifest_file.parse::<DocumentMut>().unwrap();
    let metadatas = manifest
        .get("package")
        .unwrap()
        .get("metadata")
        .as_ref()
        .unwrap()["example"]
        .clone();

    manifest["example"]
        .as_array_of_tables()
        .unwrap()
        .iter()
        .flat_map(|val| {
            let technical_name = val.get("name").unwrap().as_str().unwrap().to_string();
            if panic_on_missing && metadatas.get(&technical_name).is_none() {
                panic!("Missing metadata for example {technical_name}");
            }
            if panic_on_missing && val.get("doc-scrape-examples").is_none() {
                panic!("Example {technical_name} is missing doc-scrape-examples");
            }

            if metadatas
                .get(&technical_name)
                .and_then(|metadata| metadata.get("hidden"))
                .and_then(Item::as_bool)
                .unwrap_or(false)
            {
                return None;
            }

            metadatas.get(&technical_name).map(|metadata| Example {
                technical_name,
                path: val["path"].as_str().unwrap().to_string(),
                name: metadata["name"].as_str().unwrap().to_string(),
                description: metadata["description"].as_str().unwrap().to_string(),
                category: metadata["category"].as_str().unwrap().to_string(),
                wasm: metadata["wasm"].as_bool().unwrap(),
            })
        })
        .collect()
}

fn parse_categories() -> HashMap<Box<str>, String> {
    let manifest_file = std::fs::read_to_string("Cargo.toml").unwrap();
    let manifest = manifest_file.parse::<DocumentMut>().unwrap();
    manifest
        .get("package")
        .unwrap()
        .get("metadata")
        .as_ref()
        .unwrap()["example_category"]
        .clone()
        .as_array_of_tables()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v.get("name").unwrap().as_str().unwrap().into(),
                v.get("description").unwrap().as_str().unwrap().to_string(),
            )
        })
        .collect()
}

pub(crate) fn check(what_to_run: Command) {
    let examples = parse_examples(what_to_run.contains(Command::CHECK_MISSING));

    if what_to_run.contains(Command::UPDATE) {
        let categories = parse_categories();
        let examples_by_category: HashMap<Box<str>, Category> = examples
            .into_iter()
            .fold(HashMap::<Box<str>, Vec<Example>>::new(), |mut v, ex| {
                v.entry_ref(ex.category.as_str()).or_default().push(ex);
                v
            })
            .into_iter()
            .map(|(key, mut examples)| {
                examples.sort();
                let description = categories.get(&key).cloned();
                (
                    key,
                    Category {
                        description,
                        examples,
                    },
                )
            })
            .collect();

        let mut context = Context::new();
        context.insert("all_examples", &examples_by_category);
        Tera::new("docs-template/*.md.tpl")
            .expect("error parsing template")
            .render_to(
                "EXAMPLE_README.md.tpl",
                &context,
                File::create("examples/README.md").expect("error creating file"),
            )
            .expect("error rendering template");
    }
}
