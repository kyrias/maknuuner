use std::{borrow::Cow, env, sync::Mutex, time::Instant};

use anyhow::Context as _;
use tokio::net::TcpListener;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt as _},
    context::CxBuilder,
    router::{
        Body, IntoResponse, Next, Response, Router, RouterBuilderDiscoverExt, StatusCode,
        error::{BadRequestError, NotFoundError, RedirectError, not_found, redirect},
        layer, layout, page, uri,
    },
    view::view,
};
use tracing::Instrument;
use uuid::Uuid;

use crate::lexicon::Lexicon;

mod search;
mod styles;
mod title;

pub(super) async fn start(asset_bundle: AssetBundle, lexicon: Lexicon) -> anyhow::Result<()> {
    let host: Cow<'static, str> = env::var("HOST")
        .map(Cow::from)
        .unwrap_or("127.0.0.1".into());
    let port = if let Ok(port) = env::var("PORT") {
        port.parse::<u16>()
            .context("PORT did not contain a valid port number")?
    } else {
        3000
    };

    let listener = TcpListener::bind((host.as_ref(), port))
        .await
        .with_context(|| format!("Could not set up TCP listener on {host}:{port}"))?;
    tracing::info!("Listening on http://{host}:{port}");

    topcoat::serve(
        listener,
        Router::builder()
            .discover()
            .app_context(lexicon)
            .assets(asset_bundle)
            .build(),
    )
    .await
    .context("Failed to start topcoat")
}

// We mix tracing and title setting into one layer because the topcoat discover feature doesn't
// support multiple layers at the same level.
#[layer("/")]
async fn root_layer(cx: &mut CxBuilder, body: Body, next: Next<'_>) -> Result<Response> {
    let start = Instant::now();

    let span = tracing::info_span!(
        "request",
        request_id = %Uuid::now_v7().as_hyphenated()
    );
    tracing::debug!(parent: &span, "started processing request");

    // Insert Title into context to allow pages to set the title.
    cx.insert(Mutex::<Option<title::Title>>::default());

    let response = next
        .run(cx, body)
        .instrument(span.clone())
        .await
        .into_response(cx)
        .unwrap();
    let elapsed = start.elapsed();
    let duration = format!("{} ms", elapsed.as_millis());

    let status = response.status();
    if status.is_informational() || status.is_success() {
        tracing::info!(
            parent: &span,
            %duration,
            status = status.as_u16(),
            "finished processing request",
        );
    } else if status == StatusCode::NOT_FOUND {
        tracing::warn!(
            parent: &span,
            uri = %uri(cx),
            duration,
            status = status.as_u16(),
            "finished processing request",
        );
    } else {
        tracing::error!(
            parent: &span,
            uri = %uri(cx),
            duration,
            status = status.as_u16(),
            "finished processing request",
        );
    }

    Ok(response)
}

#[layout("/")]
async fn root_layout(slot: Result) -> Result {
    let content = match slot {
        // Pass redirects through.
        Err(error) if error.downcast_ref::<RedirectError>().is_some() => {
            return Err(error);
        }

        Err(error) if error.downcast_ref::<NotFoundError>().is_some() => {
            view! {
                (StatusCode::NOT_FOUND)
                <h2>"Page not found"</h2>
            }
        }
        Err(error) if error.downcast_ref::<BadRequestError>().is_some() => {
            let error = error.downcast_ref::<BadRequestError>().unwrap();
            view! {
                (StatusCode::BAD_REQUEST)
                <h2>"Bad request"</h2>
                <p>(error.description())</p>
            }
        }
        Err(error) => {
            tracing::error!(?error, "request failed");
            view! {
                (StatusCode::INTERNAL_SERVER_ERROR)
                <h2>"Internal server error"</h2>
            }
        }
        content => content,
    }?;

    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                title::title()

                topcoat::dev::script()
                topcoat::runtime::script()

                styles::styles()
            </head>
            <body>
                <header><h1>"Maknuuner"</h1></header>
                (content)
            </body>
        </html>
    }
}

#[page("/")]
async fn home() -> Result {
    Err(redirect("/search").into())
}

/// Return a page 404 on unmatched paths.
///
/// The topcoat router otherwise performs an early-return on unmatched paths leading to the root
/// layout not being able to render a proper error page.
#[page("/{*unmatched}")]
async fn unmatched() -> Result {
    Err(not_found().into())
}
