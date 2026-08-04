use std::{borrow::Cow, sync::Mutex};

use topcoat::{
    Result,
    context::{Cx, CxBuilder, request_context},
    router::{Body, Next, Response, layer},
    view::{NodeViewParts, component, view},
};

#[derive(Clone)]
pub(crate) struct Title(Cow<'static, str>);

impl Title {
    pub(crate) fn new<T: Into<Cow<'static, str>>>(string: T) -> Self {
        Self(string.into())
    }
}

impl NodeViewParts for Title {
    fn into_view_parts(self, cx: &Cx, _parts: &mut topcoat::view::PartsWriter<'_>) {
        request_context::<Mutex<Option<Title>>>(cx)
            .lock()
            .unwrap()
            .get_or_insert(self);
    }
}

#[layer("/")]
async fn title_layer(cx: &mut CxBuilder, body: Body, next: Next<'_>) -> Result<Response> {
    cx.insert(Mutex::<Option<Title>>::default());
    let response = next.run(cx, body).await?;
    Ok(response)
}

#[component]
pub(super) async fn title(cx: &Cx) -> Result {
    let text = request_context::<Mutex<Option<Title>>>(cx)
        .lock()
        .unwrap()
        .clone()
        .map(|title| (title.0.to_string(), Some(" · Maknuuner")))
        .unwrap_or_else(|| ("Maknuuner".to_string(), None));

    view! { <title>(text)</title> }
}
