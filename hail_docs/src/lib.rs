use std::{collections::HashMap, sync::Arc};

use hail::{FunctionInfo, Module};

/// Generate all documentation available when using the given module.
///
/// Returns a Path -> Document HashMap
pub fn generate_docs(module: Arc<Module>) -> HashMap<String, String> {
    let mut documents: HashMap<String, String> = Default::default();

    for (type_name, type_value) in &module.typing {
        let mut typing_document = String::new();
        // Header
        typing_document.push_str(&format!("# [{type_name}](#{type_name})\n"));

        // Description
        typing_document.push_str("TODO: Type value description\n");

        // Globals
        for (name, value) in module.globals.iter().filter(|v| &v.1.0 == type_value) {
            typing_document.push_str(name);
            typing_document.push_str("\n");
        }

        // Functions
        typing_document.push_str("## Functions\n");
        for function in module
            .functions
            .iter()
            .map(|v| v.1)
            .flatten()
            .filter(|v| v.return_type.as_ref().is_some_and(|v| v == type_value))
        {
            typing_document.push_str(&get_function_entry(function));
            typing_document.push_str("\n");
        }

        // Methods
        typing_document.push_str("## Methods\n");
        for method in module
            .methods
            .iter()
            .map(|v| v.1)
            .flatten()
            .filter(|v| v.param_types.first().is_some_and(|v| v == type_value))
        {
            typing_document.push_str(&get_function_entry(method));
        }

        documents.insert(format!("{type_name}.md"), typing_document);
    }

    documents
}

fn get_function_entry(info: &FunctionInfo) -> String {
    let param_names = info
        .param_names
        .iter()
        .zip(&info.param_types)
        .map(|(name, typed)| format!("{name}: [`{}`](./{})", typed.to_string(), typed.to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = info
        .return_type
        .as_ref()
        .map(|v| format!(" -> [`{}`](./{})", v.to_string(), v.to_string()))
        .unwrap_or("".to_string());
    let name = &info.name;
    let comments = info.doc_comments.join("\n");

    format!("### [{name}](#{name}) ({param_names}){return_type}\n{comments}\n")
}
