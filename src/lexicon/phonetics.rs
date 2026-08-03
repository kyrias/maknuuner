use anyhow::{Context, Result, ensure};
use compact_str::CompactString;

use crate::string::InternedString;

pub(crate) fn caphipp_to_ipa<T: AsRef<str>>(string: T) -> Result<Vec<InternedString>> {
    let string = string.as_ref();

    if string.contains(',') {
        let mut out = Vec::new();
        for results in string.split(',').map(caphipp_to_ipa) {
            for result in results? {
                if !out.contains(&result) {
                    out.push(result);
                }
            }
        }
        return Ok(out);
    }

    let mut out = vec![CompactString::default()];

    let mut alternates: Option<Vec<Phoneme>> = None;
    for segment in string.split(' ').filter(|segment| !segment.is_empty()) {
        if segment.contains("||") {
            let phonemes = segment
                .split("||")
                .map(Phoneme::try_from)
                .collect::<Result<Vec<_>>>()
                .context("Could not parse CAPHI++ alternation segment")?;

            let alternates = if let Some(alternates) = &alternates {
                ensure!(
                    alternates == &phonemes,
                    "All alternation groups must be identical"
                );
                alternates
            } else {
                for _ in 1..phonemes.len() {
                    out.push(out.first().cloned().unwrap());
                }

                alternates.insert(phonemes)
            };

            for (idx, phoneme) in alternates.iter().enumerate() {
                out[idx].push_str(phoneme.to_ipa());
            }
        } else {
            let phoneme = Phoneme::try_from(segment).context("Could not parse CAPHI++ segment")?;
            for s in &mut out {
                s.push_str(phoneme.to_ipa());
            }
        }
    }

    Ok(out.into_iter().map(InternedString::new).collect())
}

macro_rules! def_phoneme {
    ($(($ident:ident, $caphipp:literal, $ipa:literal),)*) => {
        #[allow(non_camel_case_types)]
        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub(crate) enum Phoneme {
            $($ident,)*
        }

        impl Phoneme {
            // This is used in the test case to make sure that everything roundtrip correctly and
            // there are no typos.
            #[allow(unused)]
            pub(crate) fn to_caphipp(self) -> &'static str {
                match self {
                    $(Self::$ident => $caphipp,)*
                }
            }

            pub(crate) fn to_ipa(self) -> &'static str {
                match self {
                    $(Self::$ident => $ipa,)*
                }
            }
        }

        impl TryFrom<&str> for Phoneme {
            type Error = anyhow::Error;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                let phoneme = match value {
                    $($caphipp => Self::$ident,)*
                    _ => anyhow::bail!("Unknown CAPHI++ symbol {value:?}"),
                };

                Ok(phoneme)
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn caphipp_roundtrips() {
                $(
                    assert_eq!(Phoneme::$ident.to_caphipp(), $caphipp);
                    assert_eq!(Phoneme::try_from($caphipp).unwrap(), Phoneme::$ident);
                )*
            }
        }
    };
}

def_phoneme! {
    // Standard CAPHI.
    (_2,     "2",   "ʔ"),
    (_3,     "3",   "ʕ"),
    (_7,     "7",   "ħ"),
    (a,      "a",   "a"),
    (aa,     "aa",  "aː"),
    (b,      "b",   "b"),
    (b_dot,  "b.",  "bˤ"),
    (d,      "d",   "d"),
    (d_dot,  "d.",  "dˤ"),
    (dh,     "dh",  "ð"),
    (dh_dot, "dh.", "ðˤ"),
    (dj,     "dj",  "dʒ"),
    (dz,     "dz",  "d͡z"),
    (e,      "e",   "e"),
    (ee,     "ee",  "eː"),
    (f,      "f",   "f"),
    (ff,     "ff",  "fˤ"),
    (g,      "g",   "ɡ"),
    (gh,     "gh",  "ɣ"),
    (gy,     "gy",  "ɡʲ"),
    (h,      "h",   "h"),
    (i,      "i",   "i"),
    (ii,     "ii",  "i."),
    (j,      "j",   "ʒ"),
    (k,      "k",   "k"),
    (kh,     "kh",  "x"),
    (l,      "l",   "l"),
    (l_dot,  "l.",  "lˤ"),
    (m,      "m",   "m"),
    (m_dot,  "m.",  "mˤ"),
    (n,      "n",   "n"),
    (n_dot,  "n.",  "nˤ"),
    (o,      "o",   "o"),
    (oo,     "oo",  "oː"),
    (p,      "p",   "p"),
    (p_dot,  "p.",  "pˤ"),
    (q,      "q",   "q"),
    (qh,     "qh",  "ɢ"),
    (r,      "r",   "r"),
    (r_dot,  "r.",  "rˤ"),
    (s,      "s",   "s"),
    (s_dot,  "s.",  "sˤ"),
    (sh,     "sh",  "ʃ"),
    (t,      "t",   "t"),
    (t_dot,  "t.",  "tˤ"),
    (th,     "th",  "θ"),
    (ts,     "ts",  "ts"),
    (tsh,    "tsh", "tʃ"),
    (u,      "u",   "u"),
    (uu,     "uu",  "uː"),
    (v,      "v",   "v"),
    (w,      "w",   "w"),
    (y,      "y",   "j"),
    (z,      "z",   "z"),
    (z_dot,  "z.",  "zˤ"),
    (hash,   "#",   " "),

    // Maknuune's CAPHI++ extensions.
    (D,      "D",   "(d)"),
    (D_dot,  "D.",   "(dˤ)"),
    (J,      "J",   "(dʒ)"),
    (K,      "K",   "(k)"),
    (Q,      "Q",   "(q)"),
    (S,      "S",   "(θ)"),
    (T,      "T",   "(t)"),
    (Z,      "Z",   "(ð)"),
    (Z_dot,  "Z.",  "(ðˤ)"),

    // These ones aren't in their table and CAMel explicitly says 'aa.' doesn't exist, but they
    // exist in the Maknuune dataset, so let's assume this is what was meant.
    (a_dot,  "a.",  "ɑ"),
    (aa_dot, "aa.", "ɑː"),
}
