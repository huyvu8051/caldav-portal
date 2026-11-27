use leptos::prelude::*;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <h1 class="text-title font-semibold mb-2">"Not found"</h1>
        <p class="text-body text-ink-48 mb-6">"That page does not exist."</p>
        <a
            href="/"
            role="button"
            data-testid="home"
            class="w-full h-12 inline-flex items-center justify-center rounded-full
                   bg-canvas border border-hairline text-action text-body no-underline"
        >
            "Back"
        </a>
    }
}
