use std::{cmp::Ordering, time::Instant};

use anyhow::Context as _;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt as _},
    context::{Cx, app_context},
    router::{Router, RouterBuilderDiscoverExt as _, error::redirect, layout, page, query_params},
    runtime::{Event, shard},
    view::{component, view},
};

use crate::{
    lexicon::{Definition, Lemma, Lexicon, Phrase, pos::PartOfSpeech},
    query::Query,
};

pub(super) async fn start(lexicon: Lexicon) -> anyhow::Result<()> {
    topcoat::start(
        Router::builder()
            .discover()
            .app_context(lexicon)
            .assets(AssetBundle::load().context("Failed to load asset bundle")?)
            .build(),
    )
    .await
    .context("Failed to start topcoat")
}

#[layout("/")]
async fn root_layout(slot: Result) -> Result {
    view! {
        <!DOCTYPE html>
        <html>
            <head>
                topcoat::dev::script()
                topcoat::runtime::script()
            </head>
            <body>
                <nav>
                    <a href="/">"Home"</a>
                    <a href="/search">"Search"</a>
                </nav>
                (slot?)
            </body>
        </html>
    }
}

#[page("/")]
async fn home() -> Result {
    Err(redirect("/search").into())
}

fn render_pos(pos: PartOfSpeech) -> (&'static str, Option<&'static str>) {
    use crate::lexicon::pos::{
        noun::{Noun, NounFeature},
        verb::{Verb, VerbFeature},
    };

    fn fmt_noun_feature(nf: NounFeature) -> &'static str {
        match nf {
            NounFeature::Singular => "[sg.]",
            NounFeature::MasculineSingular => "[m.sg.]",
            NounFeature::FeminineSingular => "[f.pl.]",
            NounFeature::Dual => "[d.]",
            NounFeature::Plural => "[pl.]",
            NounFeature::MasculinePlural => "[m.pl.]",
            NounFeature::FemininePlural => "[f.pl.]",
        }
    }

    fn fmt_verb_feature(vf: VerbFeature) -> &'static str {
        match vf {
            VerbFeature::Perfective => "[p.]",
            VerbFeature::Command => "[c.]",
            VerbFeature::Imperfective => "[i.]",
        }
    }

    match pos {
        PartOfSpeech::Noun(noun) => {
            let (kind, nf) = match noun {
                Noun::Plain(nf) => ("Noun", nf),
                Noun::Active(nf) => ("Noun (active participle deverbal)", nf),
                Noun::Passive(nf) => ("Noun (passive participle deverbal)", nf),
                Noun::Proper(nf) => ("Noun (proper)", nf),
                Noun::Number(nf) => ("Noun (number)", nf),
                Noun::Quantifier(nf) => ("Noun (quantifier)", nf),
            };
            (kind, nf.map(fmt_noun_feature))
        }
        PartOfSpeech::Verb(verb) => match verb {
            Verb::Plain(vf) => ("Verb", Some(fmt_verb_feature(vf))),
            Verb::Nominal => ("Verb (nominal)", None),
            Verb::Pseudo => ("Verb (pseudo)", None),
        },
    }
}

#[component]
async fn render_definition(definition: &Definition, render_glosses: bool) -> Result {
    view! {
        <li id=(("def-", definition.id)) class="definition">
            <span lang="ar">(&definition.form.raw)</span>
            if let Some(pos) = &definition.custom.pos {
                <span>
                    let (pos, feat) = render_pos(*pos);
                    " "
                    (pos)
                    " "
                    (feat)
                </span>
            }

            if render_glosses {
                <ol>
                    for gloss in &definition.glosses {
                        <li>(&gloss.raw)</li>
                    }
                </ol>
            }
        </li>
    }
}

#[component]
async fn render_phrase(phrase: &Phrase) -> Result {
    view! {
        <li id=(("ph-", phrase.id)) class="phrase">
            <span lang="ar">(&phrase.form.raw)</span>
            <ol>
                for gloss in &phrase.glosses {
                    <li>(&gloss.raw)</li>
                }
            </ol>
        </li>
    }
}

#[component]
async fn single_result(lemma: &Lemma, rank: f64, raw: bool) -> Result {
    // State used to only render the list of glosses if they differ from the previously rendered
    // definition.
    let mut first_entry = true;
    let mut glosses = lemma
        .definitions
        .first()
        .map(|def| &def.glosses)
        .or_else(|| lemma.phrases.first().map(|ph| &ph.glosses))
        .unwrap();

    view! {
        <li id=(("lemma-", lemma.lowest_id())) class="lemma">
            <span lang="ar">
                "("
                (&*lemma.root.raw)
                ") "
                (&lemma.lemma.raw)
            </span>

            if !lemma.definitions.is_empty() {
                <div>
                    <p>"Definitions"</p>
                    <ol>
                        for entry in lemma.definitions.iter() {
                            let render_glosses = first_entry || glosses != &entry.glosses;
                            let _ = glosses = &entry.glosses;
                            let _ = first_entry = false;
                            render_definition(
                                definition: entry,
                                render_glosses: render_glosses
                            )
                        }
                    </ol>
                </div>
            }

            if !lemma.phrases.is_empty() {
                <div>
                    <p>"Phrases"</p>
                    <ol>
                        for phrase in &lemma.phrases {
                            render_phrase(phrase: phrase)
                        }
                    </ol>
                </div>
            }

            if raw {
                <div>
                    <p>
                        "Result has rank: "
                        (format!("{rank:0.5}"))
                    </p>
                    <pre>(format!("{lemma:#?}"))</pre>
                </div>
            }
        </li>
    }
}

#[shard]
async fn search_results(cx: &Cx, query: String, raw: bool) -> Result {
    let query = Query::parse(&query).context("Failed to parse query")?;

    let lexicon: &Lexicon = app_context(cx);

    let instant = Instant::now();
    let mut results: Vec<_> = lexicon.search(&query).collect();
    let elapsed = instant.elapsed();

    fn comp_f64(a: &f64, b: &f64) -> Ordering {
        if a < b {
            return Ordering::Less;
        } else if a > b {
            return Ordering::Greater;
        }
        Ordering::Equal
    }
    results.sort_by(|(_, a), (_, b)| comp_f64(a, b).reverse());

    let results = results.into_iter().take(100);

    view! {
        <ol>
            for (result, rank) in results {
                single_result(lemma: result, rank: rank, raw: raw)
            }
        </ol>
        if raw {
            <div>
                "The following query took "
                (elapsed.as_secs_f64())
                "s to execute:"
                <pre>(format!("{query:#?}"))</pre>
            </div>
        }
    }
}

#[query_params(error = bad_request)]
struct SearchQuery {
    query: Option<String>,
    raw: Option<bool>,
}

#[page("/search")]
async fn search(cx: &Cx) -> Result {
    let params = query_params::<SearchQuery>(cx)?;

    let query = params
        .query
        .clone()
        .unwrap_or_else(|| r#"gloss:^money analysis:"^NOUN:P$""#.to_string());
    let raw = params.raw.unwrap_or(false);

    view! {
        signal query = query;

        <input
            :value=$(query.get())
            @input=$(|e: Event| {
                let value = e.target.value;
                query.set(value);
                raw!(
                    r#"window.history.replaceState({}, '',
                    '?' + new URLSearchParams({ "query": ${value}, "raw": ${raw} }
                ).toString())"#
                );
            })
        >

        search_results(query: $(query.get()), raw: $(raw))
    }
}
