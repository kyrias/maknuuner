use std::{cmp::Ordering, time::Instant};

use anyhow::Context;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, query_params},
    runtime::{Event, shard},
    view::{Unescaped, View, component, view},
};

use crate::{
    lexicon::{Definition, Lemma, Lexicon, Phrase, Transcription, pos::PartOfSpeech},
    query::Query,
    string::SearchableString,
    web::title::Title,
};

#[query_params(error = bad_request)]
struct SearchQuery {
    query: Option<String>,
    raw: Option<bool>,
}

#[page("/search")]
pub(super) async fn search(cx: &Cx) -> Result {
    let params = query_params::<SearchQuery>(cx)?;

    let query = params
        .query
        .clone()
        .unwrap_or_else(|| r#"gloss:^money analysis:"^NOUN:P$""#.to_string());
    let raw = params.raw.unwrap_or(false);

    view! {
        signal query = query;

        (Title::new("Search"))

        <h2>"Search"</h2>

        <input
            class="search"
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
        <div class="results">
            for (lemma, rank) in results {
                result(lemma: lemma, rank: rank, raw: raw)
            }
        </div>
        if raw {
            <div class="raw">
                "The following query took "
                (elapsed.as_secs_f64())
                "s to execute:"
                <pre>(format!("{query:#?}"))</pre>
            </div>
        }
    }
}

#[component]
async fn result(lemma: &Lemma, rank: f64, raw: bool) -> Result {
    // State used to only render the list of glosses if they differ from the previously rendered
    // definition.
    let mut previous_glosses = lemma
        .definitions
        .first()
        .map(|def| &def.glosses_english)
        .or_else(|| lemma.phrases.first().map(|ph| &ph.glosses_english))
        .unwrap();

    let root_title = match lemma.root {
        crate::lexicon::Root::Root(_) => "Root",
        crate::lexicon::Root::NonTemplaticWordStem(_) => "Root (non-templatic word stem)",
    };

    view! {
        <div id=(("lemma-", lemma.lowest_id)) class="result">
            <h3 lang="ar-PS">
                <span class="root" title=(root_title)>
                    "("
                    (&*lemma.root)
                    ")"
                </span>
                <span class="lemma" title="Lemma">(&lemma.lemma)</span>

                <a href=(("#lemma-", lemma.lowest_id))>
                    (Unescaped::new_unchecked("&sect;"))
                </a>
            </h3>

            <div class="inner">
                if !lemma.definitions.is_empty() {
                    <div class="definitions">
                        <h4>"Definitions"</h4>
                        <ol>
                            for (idx, entry) in lemma.definitions.iter().enumerate() {
                                let glosses = if idx == 0
                                    || previous_glosses != &entry.glosses_english
                                {
                                    entry.glosses_english.as_slice()
                                } else {
                                    &[]
                                };
                                let _ = previous_glosses = &entry.glosses_english;
                                single_definition(definition: entry, glosses: glosses)
                            }
                        </ol>
                    </div>
                }

                if !lemma.phrases.is_empty() {
                    <div class="phrases">
                        <h4>"Phrases"</h4>
                        <ol>
                            for phrase in &lemma.phrases {
                                single_phrase(phrase: phrase)
                            }
                        </ol>
                    </div>
                }

                if raw {
                    <div class="raw">
                        <p>
                            "Result has rank: "
                            (format!("{rank:0.5}"))
                        </p>
                        <pre>(format!("{lemma:#?}"))</pre>
                    </div>
                }
            </div>
        </div>
    }
}

#[component]
async fn single_definition(definition: &Definition, glosses: &[SearchableString]) -> Result {
    view! {
        <li id=(("def-", definition.id)) class="definition">
            <div class="form-header">
                <span lang="ar-PS">(&definition.form)</span>
                transcription(trans: &definition.transcription)
                part_of_speech(pos: definition.pos)
                glosses_msa(glosses: &definition.glosses_msa)
            </div>
            glosses_english(glosses: glosses)
        </li>
    }
}

#[component]
async fn single_phrase(phrase: &Phrase) -> Result {
    view! {
        <li id=(("ph-", phrase.id)) class="phrase">
            <div class="form-header">
                <span lang="ar-PS">(&phrase.form)</span>
                transcription(trans: &phrase.transcription)
                glosses_msa(glosses: &phrase.glosses_msa)
            </div>
            glosses_english(glosses: &phrase.glosses_english)
        </li>
    }
}

#[component]
async fn transcription(trans: &Transcription) -> Result {
    view! {
        <span class="transcription" title="IPA transcription">
            <span>"("</span>
            <span class="list">
                let num = trans.ipa.len();
                for (idx, ipa) in trans.ipa.iter().enumerate() {
                    <span>
                        "/\u{2060}"
                        (ipa)
                        "\u{2060}/"
                    </span>
                    if (idx + 1) < num {
                        ", "
                    }
                }
            </span>
            <span>")"</span>
        </span>
    }
}

#[component]
async fn part_of_speech(pos: Option<PartOfSpeech>) -> Result {
    let Some(pos) = pos else {
        return Ok(View::empty());
    };

    use crate::lexicon::pos::{
        noun::{Noun, NounFeature},
        verb::{Verb, VerbFeature},
    };

    fn noun_feature(nf: NounFeature) -> (&'static str, &'static str) {
        match nf {
            NounFeature::Singular => ("[sg.]", "Singular"),
            NounFeature::MasculineSingular => ("[m.sg.]", "Masculine singular"),
            NounFeature::FeminineSingular => ("[f.pl.]", "Feminine plural"),
            NounFeature::Dual => ("[d.]", "Dual"),
            NounFeature::Plural => ("[pl.]", "Plural"),
            NounFeature::MasculinePlural => ("[m.pl.]", "Masculine plural"),
            NounFeature::FemininePlural => ("[f.pl.]", "Feminine plural"),
        }
    }

    fn verb_feature(vf: VerbFeature) -> (&'static str, &'static str) {
        match vf {
            VerbFeature::Perfective => ("[p.]", "Perfective"),
            VerbFeature::Command => ("[c.]", "Command"),
            VerbFeature::Imperfective => ("[i.]", "Imperfective"),
        }
    }

    let (pos, feature) = match pos {
        PartOfSpeech::Noun(noun) => {
            let (kind, nf) = match noun {
                Noun::Plain(nf) => ("Noun", nf),
                Noun::Active(nf) => ("Noun (active participle deverbal)", nf),
                Noun::Passive(nf) => ("Noun (passive participle deverbal)", nf),
                Noun::Proper(nf) => ("Noun (proper)", nf),
                Noun::Number(nf) => ("Noun (number)", nf),
                Noun::Quantifier(nf) => ("Noun (quantifier)", nf),
            };
            (kind, nf.map(noun_feature))
        }
        PartOfSpeech::Verb(verb) => match verb {
            Verb::Plain(vf) => ("Verb", Some(verb_feature(vf))),
            Verb::Nominal => ("Verb (nominal)", None),
            Verb::Pseudo => ("Verb (pseudo)", None),
        },
    };

    let feature = if let Some((feature, tooltip)) = feature {
        view! { <span title=(tooltip)>(feature)</span> }
    } else {
        view! {}
    };

    view! {
        <span class="pos">
            (pos)
            " "
            (feature?)
        </span>
    }
}

#[component]
async fn glosses_msa(glosses: &[SearchableString]) -> Result {
    view! {
        if !glosses.is_empty() {
            <span class="msa-glosses">
                let num = glosses.len();
                <span>"("</span>
                "msa. "
                <span class="list">
                    for (idx, gloss) in glosses.iter().enumerate() {
                        <span>
                            <span lang="ar-PS" dir="rtl">(gloss)</span>
                            if (idx + 1) < num {
                                ", "
                            }
                        </span>
                    }
                </span>
                <span>")"</span>
            </span>
        }
    }
}

#[component]
async fn glosses_english(glosses: &[SearchableString]) -> Result {
    view! {
        if !glosses.is_empty() {
            <ol class="glosses">
                for gloss in glosses {
                    <li>(gloss)</li>
                }
            </ol>
        }
    }
}
