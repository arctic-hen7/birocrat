use leptos::{component, view, IntoView};
use wasm_bindgen::{prelude::*, JsCast};

/// Mounts Birocrat at the provided ID. This will return `true` if mounting was successful, and
/// `false` otherwise.
#[wasm_bindgen]
pub fn birocrat(id: &str) -> bool {
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id(id);
    if let Some(root) = root {
        let root = root.dyn_into::<web_sys::HtmlElement>().unwrap();
        leptos::mount_to(root, || view! { <App /> });

        true
    } else {
        false
    }
}

#[component]
fn App() -> impl IntoView {
    view! {}
}
