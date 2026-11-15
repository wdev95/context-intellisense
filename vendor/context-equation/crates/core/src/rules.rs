//! Ids of the rules in `spec/rules.md`. The code cites the rule it applies,
//! and the CLI's `--explain` shows which rules changed the result.

pub const L_NUM_1: &str = "L-NUM-1";
pub const L_NUM_2: &str = "L-NUM-2";
pub const L_NUM_3: &str = "L-NUM-3";
pub const L_FUN_1: &str = "L-FUN-1";
pub const L_LET_1: &str = "L-LET-1";
pub const L_ESP_1: &str = "L-ESP-1";
pub const L_ESP_2: &str = "L-ESP-2";
pub const L_INT_1: &str = "L-INT-1";
pub const L_UNI_1: &str = "L-UNI-1";
pub const L_UNI_2: &str = "L-UNI-2";
pub const L_UNI_3: &str = "L-UNI-3";
pub const L_CHE_1: &str = "L-CHE-1";
pub const L_CHE_2: &str = "L-CHE-2";
pub const L_ENT_1: &str = "L-ENT-1";
pub const L_ENT_2: &str = "L-ENT-2";
pub const L_ENT_3: &str = "L-ENT-3";
pub const L_ENT_4: &str = "L-ENT-4";
pub const L_MAC_1: &str = "L-MAC-1";
pub const L_TXT_1: &str = "L-TXT-1";

// Recovery from invalid LaTeX in tolerant mode (spec/tolerance.md)
pub const E_REC_1: &str = "E-REC-1";
pub const E_REC_2: &str = "E-REC-2";
pub const E_REC_3: &str = "E-REC-3";
pub const E_REC_4: &str = "E-REC-4";
pub const E_REC_5: &str = "E-REC-5";
pub const E_REC_6: &str = "E-REC-6";
pub const E_REC_7: &str = "E-REC-7";
pub const E_REC_8: &str = "E-REC-8";
pub const E_REC_9: &str = "E-REC-9";
pub const E_REC_10: &str = "E-REC-10";
// Output in braille (spec/braille.md)
pub const S_BRL_1: &str = "S-BRL-1";
// Output in MathML (spec/mathml.md)
pub const S_MML_1: &str = "S-MML-1";
pub const L_CONTA_1: &str = "L-CONTA-1";
pub const L_CONTA_2: &str = "L-CONTA-2";
// Automatic line breaking (spec/rules.md)
pub const L_BRK_1: &str = "L-BRK-1";
pub const L_BRK_2: &str = "L-BRK-2";

/// A rule that acted on this equation, with the affected snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub rule: &'static str,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct Registry {
    pub applied: Vec<Applied>,
    /// Input problems the tolerant mode worked around.
    pub warnings: Vec<Applied>,
}

impl Registry {
    pub fn note(&mut self, rule: &'static str, description: impl Into<String>) {
        self.applied.push(Applied { rule, description: description.into() });
    }

    pub fn warn(&mut self, rule: &'static str, description: impl Into<String>) {
        self.warnings.push(Applied { rule, description: description.into() });
    }

    /// Removes exact repeats, keeping the order of first appearance.
    pub fn dedup(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.applied.retain(|a| seen.insert((a.rule, a.description.clone())));
        let mut seen = std::collections::HashSet::new();
        self.warnings.retain(|a| seen.insert((a.rule, a.description.clone())));
    }
}
