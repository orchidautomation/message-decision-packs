use crate::artifact_hash::{
    AuthorityJsonLimits, canonical_json_sha256_for_domain, parse_authority_json,
};
use crate::run_contracts::{AssuranceEvidenceState, EvidenceProvenance, TerminalState};
use crate::value_contracts::valid_date_time;
use anyhow::{Result, anyhow};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path};

pub(crate) const CONFORMANCE_CANDIDATE_V1: &str = "mdp.conformance-candidate.v1";
pub(crate) const MODEL_INVOCATION_EVIDENCE_V1: &str = "mdp.model-invocation-evidence.v1";
pub(crate) const EVALUATOR_INVENTORY_V1: &str = "mdp.evaluator-inventory.v1";
pub(crate) const EVALUATOR_RESULT_V1: &str = "mdp.evaluator-result.v1";
pub(crate) const PRIVATE_RECORD_POLICY_V1: &str = "mdp.private-record-policy.v1";
pub(crate) const PUBLICATION_APPROVAL_V1: &str = "mdp.publication-approval.v1";
pub(crate) const CONFORMANCE_TRIAL_V1: &str = "mdp.conformance-trial.v1";
pub(crate) const JOB_CONFORMANCE_V1: &str = "mdp.job-conformance.v1";
pub(crate) const CONFORMANCE_REPORT_V1: &str = "mdp.conformance-report.v1";
pub(crate) const PUBLIC_CONFORMANCE_REPORT_V1: &str = "mdp.public-conformance-report.v1";
pub(crate) const DETERMINISTIC_CONFORMANCE_V1: &str = "mdp.deterministic-conformance.v1";
pub(crate) const CONFORMANCE_VERIFIER_RECEIPT_V1: &str = "mdp.conformance-verifier-receipt.v1";

pub(crate) const MAX_CONFORMANCE_AUTHORITY_BYTES: usize = 1_048_576;
pub(crate) const MAX_CONFORMANCE_DEPTH: usize = 32;
pub(crate) const MAX_CONFORMANCE_OBJECT_MEMBERS: usize = 512;
pub(crate) const MAX_CONFORMANCE_ARRAY_ITEMS: usize = 256;
pub(crate) const MAX_CONFORMANCE_STRING_BYTES: usize = 16_384;
pub(crate) const MAX_MODEL_VISIBLE_INPUTS: usize = 64;
pub(crate) const MAX_CANDIDATE_AUTHORITIES: usize = 64;
pub(crate) const MAX_TRIALS_PER_JOB: usize = 128;
pub(crate) const MAX_JOURNEY_LINKS: usize = 64;
pub(crate) const REQUIRED_COLD_TRIALS: usize = 3;

pub(crate) fn conformance_limits() -> AuthorityJsonLimits {
    AuthorityJsonLimits {
        max_bytes: MAX_CONFORMANCE_AUTHORITY_BYTES,
        max_depth: MAX_CONFORMANCE_DEPTH,
        max_object_members: MAX_CONFORMANCE_OBJECT_MEMBERS,
        max_array_length: MAX_CONFORMANCE_ARRAY_ITEMS,
        max_string_bytes: MAX_CONFORMANCE_STRING_BYTES,
    }
}

pub(crate) trait ConformanceContract: Serialize {
    const CONTRACT: &'static str;
    fn validate(&self) -> Result<()>;
}

pub(crate) fn hash_authority_value(domain: &str, value: &Value) -> Result<String> {
    canonical_json_sha256_for_domain(domain, value)
}

pub(crate) fn canonical_authority_sha256<T: ConformanceContract>(authority: &T) -> Result<String> {
    authority.validate()?;
    canonical_json_sha256_for_domain(T::CONTRACT, &serde_json::to_value(authority)?)
}

/// Digest of the candidate facts frozen before the evaluator inventory is
/// attached. Excluding the inventory binding avoids a circular hash graph.
pub(crate) fn candidate_freeze_sha256(candidate: &ConformanceCandidateV1) -> Result<String> {
    candidate.validate()?;
    let mut frozen = candidate.clone();
    frozen.evaluator_inventory_sha256.clear();
    frozen
        .authorities
        .retain(|authority| authority.role != CandidateAuthorityRole::EvaluatorInventory);
    hash_authority_value(
        "mdp.conformance-candidate-freeze.v1",
        &serde_json::to_value(frozen)?,
    )
}

fn parse_contract<T: ConformanceContract + DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let authority: T = parse_authority_json(bytes, conformance_limits())?;
    authority.validate()?;
    Ok(authority)
}

pub(crate) fn parse_candidate(bytes: &[u8]) -> Result<ConformanceCandidateV1> {
    parse_contract(bytes)
}

/// U2 seam: reads a candidate only after proving that the manifest itself is a
/// single regular file contained by the caller-selected staged artifact root.
pub(crate) fn parse_candidate_file(
    path: &Path,
    artifact_root: &Path,
) -> Result<ConformanceCandidateV1> {
    let relative = path
        .strip_prefix(artifact_root)
        .map_err(|_| anyhow!("candidate manifest must be named beneath staged artifact root"))?;
    let candidate = parse_candidate(&read_contained_file(artifact_root, relative)?)?;
    validate_relative_path(&candidate.artifact_root)?;
    Ok(candidate)
}

/// Reads an untrusted authority through the shared conformance containment and
/// resource checks. The returned value is deserialized only after its exact
/// bytes have matched the candidate's declared digest.
pub(crate) fn read_contained_authority<T: DeserializeOwned>(
    artifact_root: &Path,
    relative_path: &str,
    expected_sha256: &str,
    json_limits: AuthorityJsonLimits,
) -> Result<T> {
    let bytes = read_contained_bytes_inner(
        artifact_root,
        relative_path,
        expected_sha256,
        None,
        json_limits.max_bytes,
    )?;
    parse_authority_json(&bytes, json_limits)
}

/// Raw-byte form for YAML and other existing authorities. Both digest and byte
/// count are checked on the one safely-opened file snapshot.
pub(crate) fn read_contained_authority_bytes(
    artifact_root: &Path,
    authority: &StagedAuthorityRef,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    read_contained_bytes_inner(
        artifact_root,
        &authority.relative_path,
        &authority.sha256,
        Some(authority.byte_count),
        max_bytes,
    )
}

/// Reads one caller-selected composite member only after proving that the path
/// is relative, regular, link-free, bounded, and contained by `artifact_root`.
/// Composite assembly computes the member's canonical authority digest from
/// this single byte snapshot instead of trusting a path or opaque identifier.
pub(crate) fn read_contained_file(artifact_root: &Path, relative_path: &Path) -> Result<Vec<u8>> {
    let relative = relative_path
        .to_str()
        .ok_or_else(|| anyhow!("authority path must be valid UTF-8"))?;
    validate_relative_path(relative)?;
    read_contained_file_snapshot(
        artifact_root,
        relative_path,
        MAX_CONFORMANCE_AUTHORITY_BYTES,
    )
}

fn read_contained_bytes_inner(
    artifact_root: &Path,
    relative_path: &str,
    expected_sha256: &str,
    expected_byte_count: Option<u64>,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    validate_relative_path(relative_path)?;
    validate_hash(expected_sha256, "expected authority sha256")?;
    let bytes = read_contained_file_snapshot(artifact_root, Path::new(relative_path), max_bytes)?;
    if expected_byte_count.is_some_and(|expected| expected != bytes.len() as u64) {
        return Err(anyhow!("authority byte count mismatch"));
    }
    if crate::artifact_hash::sha256_hex(&bytes) != expected_sha256 {
        return Err(anyhow!("authority SHA-256 mismatch"));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn read_contained_file_snapshot(root: &Path, relative: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    let mut directory = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(|error| anyhow!("cannot open staged artifact root safely: {error}"))?;
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(anyhow!("artifact path must name a file"));
    }
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(anyhow!("artifact path must be a contained relative path"));
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| anyhow!("artifact path contains an invalid NUL byte"))?;
        let final_component = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if final_component {
                0
            } else {
                libc::O_DIRECTORY
            };
        // SAFETY: both descriptors and the NUL-terminated component name remain valid for the call.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(anyhow!(
                "cannot open contained authority component safely: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: openat returned a new owned descriptor on success.
        let opened = unsafe { File::from_raw_fd(fd) };
        if final_component {
            return read_open_file_snapshot(opened, max_bytes);
        }
        directory = opened;
    }
    Err(anyhow!("artifact path must name a file"))
}

#[cfg(not(unix))]
fn read_contained_file_snapshot(root: &Path, relative: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let root = root.canonicalize()?;
    let candidate = root.join(relative);
    let resolved = candidate.canonicalize()?;
    if !resolved.starts_with(&root) {
        return Err(anyhow!("authority path escapes staged artifact root"));
    }
    read_bounded_file_snapshot(&resolved, max_bytes)
}

#[cfg(not(unix))]
fn read_bounded_file_snapshot(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(anyhow!("authority must be a regular non-symlink file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.nlink() != 1 {
            return Err(anyhow!("authority must not be hard linked"));
        }
    }
    if before.len() > max_bytes as u64 {
        return Err(anyhow!("authority exceeds {} byte limit", max_bytes));
    }
    let file = File::open(path)?;
    let opened = file.metadata()?;
    if !same_file(&before, &opened) {
        return Err(anyhow!("authority changed before read"));
    }
    read_open_file_snapshot(file, max_bytes)
}

fn read_open_file_snapshot(mut file: File, max_bytes: usize) -> Result<Vec<u8>> {
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(anyhow!("authority must be a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened.nlink() != 1 {
            return Err(anyhow!("authority must not be hard linked"));
        }
    }
    if opened.len() > max_bytes as u64 {
        return Err(anyhow!("authority exceeds {} byte limit", max_bytes));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.by_ref()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(anyhow!("authority exceeds {} byte limit", max_bytes));
    }
    let after = file.metadata()?;
    if !same_file(&opened, &after) || after.len() != bytes.len() as u64 {
        return Err(anyhow!("authority changed during read"));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

pub(crate) fn parse_invocation(bytes: &[u8]) -> Result<ModelInvocationEvidenceV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_evaluator_inventory(bytes: &[u8]) -> Result<EvaluatorInventoryV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_evaluator_result(bytes: &[u8]) -> Result<EvaluatorResultV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_lifecycle_policy(bytes: &[u8]) -> Result<PrivateRecordPolicyV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_publication_approval(bytes: &[u8]) -> Result<PublicationApprovalV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_trial(bytes: &[u8]) -> Result<ConformanceTrialV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_job_conformance(bytes: &[u8]) -> Result<JobConformanceV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_behavioral_evaluation(bytes: &[u8]) -> Result<BehavioralEvaluation> {
    parse_contract(bytes)
}
pub(crate) fn parse_deterministic_conformance(bytes: &[u8]) -> Result<DeterministicConformanceV1> {
    parse_contract(bytes)
}
pub(crate) fn parse_conformance_verifier_receipt(
    bytes: &[u8],
) -> Result<ConformanceVerifierReceiptV1> {
    parse_contract(bytes)
}

fn validate_contract(actual: &str, expected: &str) -> Result<()> {
    if actual != expected {
        return Err(anyhow!("expected contract {expected}, found {actual}"));
    }
    Ok(())
}
fn validate_nonempty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_CONFORMANCE_STRING_BYTES {
        return Err(anyhow!("{field} must be non-empty and bounded"));
    }
    Ok(())
}
fn validate_hash(value: &str, field: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(anyhow!("{field} must be a lowercase SHA-256 digest"));
    }
    Ok(())
}

fn validate_hex(value: &str, expected_len: usize, field: &str) -> Result<()> {
    if value.len() != expected_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(anyhow!("{field} must be lowercase hexadecimal"));
    }
    Ok(())
}

fn decode_hex<const N: usize>(value: &str, field: &str) -> Result<[u8; N]> {
    validate_hex(value, N * 2, field)?;
    let mut output = [0u8; N];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| anyhow!("{field} must be lowercase hexadecimal"))?;
    }
    Ok(output)
}

fn validate_public_key_identity(public_key_hex: &str, identity_sha256: &str) -> Result<()> {
    let key = decode_hex::<32>(public_key_hex, "public_key_hex")?;
    if crate::artifact_hash::sha256_hex(&key) != identity_sha256 {
        return Err(anyhow!(
            "identity authority digest does not match public key"
        ));
    }
    VerifyingKey::from_bytes(&key).map_err(|_| anyhow!("invalid Ed25519 public key"))?;
    Ok(())
}

fn verify_authority_signature<T: Clone + Serialize>(
    authority: &T,
    signature_hex: &str,
    public_key_hex: &str,
    domain: &str,
    clear_signature: impl FnOnce(&mut T),
) -> Result<()> {
    let key_bytes = decode_hex::<32>(public_key_hex, "public_key_hex")?;
    let signature_bytes = decode_hex::<64>(signature_hex, "signature_hex")?;
    let key =
        VerifyingKey::from_bytes(&key_bytes).map_err(|_| anyhow!("invalid Ed25519 public key"))?;
    let mut unsigned = authority.clone();
    clear_signature(&mut unsigned);
    let digest_hex = canonical_json_sha256_for_domain(domain, &serde_json::to_value(unsigned)?)?;
    let digest = decode_hex::<32>(&digest_hex, "signature payload digest")?;
    key.verify(&digest, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| anyhow!("Ed25519 authority signature verification failed"))
}
fn validate_relative_path(value: &str) -> Result<()> {
    validate_nonempty(value, "relative path")?;
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(anyhow!("artifact path must be a contained relative path"));
    }
    Ok(())
}
fn validate_unique<'a>(items: impl IntoIterator<Item = &'a str>, field: &str) -> Result<()> {
    let mut seen = HashSet::new();
    for item in items {
        if !seen.insert(item) {
            return Err(anyhow!("duplicate {field}: {item}"));
        }
    }
    Ok(())
}

fn validate_utc_timestamp(value: &str, field: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let punctuation = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    if bytes.len() != 20
        || punctuation
            .iter()
            .any(|(index, expected)| bytes[*index] != *expected)
        || bytes.iter().enumerate().any(|(index, byte)| {
            !punctuation.iter().any(|(position, _)| *position == index) && !byte.is_ascii_digit()
        })
        || !valid_date_time(value)
    {
        return Err(anyhow!(
            "{field} must use canonical UTC form YYYY-MM-DDTHH:MM:SSZ"
        ));
    }
    Ok(())
}

fn validate_public_reason_code(value: &str) -> Result<()> {
    const SAFE: &[&str] = &[
        "required-sampling-incomplete",
        "unreferenced-evaluator-result",
        "trial-replay-or-identity-reuse",
        "fresh-host-binding-not-verified",
        "cold-isolation-unproven",
        "model-visible-context-oracle-leak-or-hash-mismatch",
        "challenge-not-frozen-before-trial",
        "output-lifecycle-policy-mismatch",
        "protected-challenge-provenance-invalid",
        "sanitized-public-exact-hash-approval-missing",
        "missing-or-ambiguous-score",
        "sampling-threshold-not-met",
        "behavioral-trials-not-run",
    ];
    if !SAFE.contains(&value) {
        return Err(anyhow!("unsafe or unknown public reason code"));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackReleaseIdentity {
    pub(crate) pack_id: String,
    pub(crate) release_id: String,
    pub(crate) version: String,
    pub(crate) portable_digest: String,
    pub(crate) source_revision: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CandidateAuthorityRole {
    PackManifest,
    Requirements,
    ProductFoundation,
    SkillsRoute,
    Prompt,
    PromptInvocation,
    SourceLineage,
    NormalizedInput,
    RoutedContext,
    GovernedOutput,
    ClaimsValidation,
    DecisionResult,
    RunBundle,
    RunReceipt,
    RunVerification,
    EvaluatorInventory,
    PrivateRecordPolicy,
    PublicationApproval,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StagedAuthorityRef {
    pub(crate) role: CandidateAuthorityRole,
    pub(crate) contract: String,
    pub(crate) relative_path: String,
    pub(crate) sha256: String,
    pub(crate) byte_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceCandidateV1 {
    pub(crate) contract: String,
    pub(crate) candidate_id: String,
    pub(crate) artifact_root: String,
    pub(crate) job_id: String,
    pub(crate) pack_release: PackReleaseIdentity,
    pub(crate) cli_version: String,
    pub(crate) fixture_id: String,
    pub(crate) challenge_id: Option<String>,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) authorities: Vec<StagedAuthorityRef>,
    pub(crate) lifecycle_policy_sha256: String,
}
impl ConformanceContract for ConformanceCandidateV1 {
    const CONTRACT: &'static str = CONFORMANCE_CANDIDATE_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (value, name) in [
            (&self.candidate_id, "candidate_id"),
            (&self.job_id, "job_id"),
            (&self.cli_version, "cli_version"),
            (&self.fixture_id, "fixture_id"),
        ] {
            validate_nonempty(value, name)?;
        }
        validate_relative_path(&self.artifact_root)?;
        for (value, name) in [
            (&self.pack_release.portable_digest, "portable_digest"),
            (&self.pack_release.source_revision, "source_revision"),
            (
                &self.evaluator_inventory_sha256,
                "evaluator_inventory_sha256",
            ),
            (&self.lifecycle_policy_sha256, "lifecycle_policy_sha256"),
        ] {
            validate_hash(value, name)?;
        }
        if self.authorities.is_empty() || self.authorities.len() > MAX_CANDIDATE_AUTHORITIES {
            return Err(anyhow!(
                "candidate authorities must be non-empty and bounded"
            ));
        }
        validate_unique(
            self.authorities
                .iter()
                .map(|item| item.relative_path.as_str()),
            "authority path",
        )?;
        let roles: HashSet<_> = self.authorities.iter().map(|item| item.role).collect();
        for required in [
            CandidateAuthorityRole::PackManifest,
            CandidateAuthorityRole::Requirements,
            CandidateAuthorityRole::Prompt,
            CandidateAuthorityRole::EvaluatorInventory,
            CandidateAuthorityRole::PrivateRecordPolicy,
        ] {
            if !roles.contains(&required) {
                return Err(anyhow!(
                    "candidate missing required authority role: {required:?}"
                ));
            }
        }
        for authority in &self.authorities {
            validate_nonempty(&authority.contract, "authority contract")?;
            let contract_allowed = match authority.role {
                CandidateAuthorityRole::PackManifest => authority.contract == "mdp.v0",
                CandidateAuthorityRole::Requirements => matches!(
                    authority.contract.as_str(),
                    "mdp.requirements.v1" | "mdp.requirements.v2"
                ),
                CandidateAuthorityRole::SkillsRoute => authority.contract == "mdp.skills.v1",
                CandidateAuthorityRole::Prompt => matches!(
                    authority.contract.as_str(),
                    "mdp.prompt.v0" | "mdp.prompt.v1"
                ),
                CandidateAuthorityRole::PromptInvocation => {
                    authority.contract == "mdp.prompt-invocation.v1"
                }
                CandidateAuthorityRole::RoutedContext => {
                    authority.contract == "mdp.routed-context.v1"
                }
                CandidateAuthorityRole::GovernedOutput => {
                    authority.contract == "mdp.prompt-output.v0"
                }
                CandidateAuthorityRole::RunBundle => authority.contract == "mdp.run-bundle.v1",
                CandidateAuthorityRole::RunReceipt => authority.contract == "mdp.run-receipt.v1",
                CandidateAuthorityRole::RunVerification => {
                    authority.contract == "mdp.run-verification.v1"
                }
                CandidateAuthorityRole::EvaluatorInventory => {
                    authority.contract == EVALUATOR_INVENTORY_V1
                }
                CandidateAuthorityRole::PrivateRecordPolicy => {
                    authority.contract == PRIVATE_RECORD_POLICY_V1
                }
                CandidateAuthorityRole::PublicationApproval => {
                    authority.contract == PUBLICATION_APPROVAL_V1
                }
                _ => true,
            };
            if !contract_allowed {
                return Err(anyhow!(
                    "authority role has wrong contract: {:?}",
                    authority.role
                ));
            }
            validate_relative_path(&authority.relative_path)?;
            validate_hash(&authority.sha256, "authority sha256")?;
            if authority.byte_count == 0
                || authority.byte_count as usize > MAX_CONFORMANCE_AUTHORITY_BYTES
            {
                return Err(anyhow!("authority byte_count is outside limits"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InvocationPhase {
    Normalization,
    Generation,
    Review,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelVisibleInput {
    pub(crate) name: String,
    pub(crate) sha256: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FreshnessEvidence {
    pub(crate) session_id: String,
    pub(crate) resumed: bool,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) verifier_receipt_sha256: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IsolationObservation {
    pub(crate) dimension: String,
    pub(crate) state: AssuranceEvidenceState,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) limitations: Vec<String>,
    pub(crate) verifier_receipt_sha256: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderMetadata {
    pub(crate) request_id: Option<String>,
    pub(crate) region: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrivateArtifactRef {
    pub(crate) artifact_id: String,
    pub(crate) sha256: String,
    pub(crate) byte_count: u64,
    pub(crate) lifecycle_policy_sha256: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelInvocationEvidenceV1 {
    pub(crate) contract: String,
    pub(crate) invocation_id: String,
    pub(crate) trial_id: String,
    pub(crate) phase: InvocationPhase,
    pub(crate) job_id: String,
    pub(crate) fixture_id: String,
    pub(crate) candidate_sha256: String,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) prompt_sha256: String,
    pub(crate) input_artifacts: Vec<ModelVisibleInput>,
    pub(crate) model_visible_context_sha256: String,
    pub(crate) started_at: String,
    pub(crate) completed_at: String,
    pub(crate) freshness: FreshnessEvidence,
    pub(crate) isolation: Vec<IsolationObservation>,
    pub(crate) provider_metadata: Option<ProviderMetadata>,
    pub(crate) terminal_state: TerminalState,
    pub(crate) output: Option<PrivateArtifactRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceVerifierReceiptV1 {
    pub(crate) contract: String,
    pub(crate) receipt_id: String,
    pub(crate) verifier_name: String,
    pub(crate) verifier_version: String,
    pub(crate) verifier_config_sha256: String,
    pub(crate) identity_authority_sha256: String,
    pub(crate) invocation_id: String,
    pub(crate) candidate_sha256: String,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) model_visible_context_sha256: String,
    pub(crate) started_at: String,
    pub(crate) completed_at: String,
    pub(crate) freshness_verified: bool,
    pub(crate) isolation_dimensions: Vec<String>,
    pub(crate) signature_hex: String,
}
impl ConformanceVerifierReceiptV1 {
    pub(crate) fn verify_signature(&self, public_key_hex: &str) -> Result<()> {
        verify_authority_signature(
            self,
            &self.signature_hex,
            public_key_hex,
            "mdp.conformance-verifier-receipt.v1.signature.v1",
            |value| value.signature_hex.clear(),
        )
    }
}
impl ConformanceContract for ConformanceVerifierReceiptV1 {
    const CONTRACT: &'static str = CONFORMANCE_VERIFIER_RECEIPT_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        validate_nonempty(&self.receipt_id, "receipt_id")?;
        validate_nonempty(&self.verifier_name, "verifier_name")?;
        validate_nonempty(&self.verifier_version, "verifier_version")?;
        validate_nonempty(&self.invocation_id, "invocation_id")?;
        validate_hex(&self.signature_hex, 128, "signature_hex")?;
        for (hash, field) in [
            (&self.candidate_sha256, "candidate_sha256"),
            (&self.verifier_config_sha256, "verifier_config_sha256"),
            (&self.identity_authority_sha256, "identity_authority_sha256"),
            (
                &self.evaluator_inventory_sha256,
                "evaluator_inventory_sha256",
            ),
            (
                &self.model_visible_context_sha256,
                "model_visible_context_sha256",
            ),
        ] {
            validate_hash(hash, field)?;
        }
        validate_utc_timestamp(&self.started_at, "started_at")?;
        validate_utc_timestamp(&self.completed_at, "completed_at")?;
        if self.completed_at < self.started_at || !self.freshness_verified {
            return Err(anyhow!(
                "verifier receipt timing or freshness verdict is invalid"
            ));
        }
        validate_unique(
            self.isolation_dimensions.iter().map(String::as_str),
            "verifier isolation dimension",
        )?;
        if COLD_ISOLATION_DIMENSIONS.iter().any(|required| {
            !self
                .isolation_dimensions
                .iter()
                .any(|value| value == required)
        }) {
            return Err(anyhow!(
                "verifier receipt lacks required isolation dimensions"
            ));
        }
        Ok(())
    }
}
impl ConformanceContract for ModelInvocationEvidenceV1 {
    const CONTRACT: &'static str = MODEL_INVOCATION_EVIDENCE_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (value, name) in [
            (&self.invocation_id, "invocation_id"),
            (&self.trial_id, "trial_id"),
            (&self.job_id, "job_id"),
            (&self.fixture_id, "fixture_id"),
            (&self.requested_model, "requested_model"),
            (&self.resolved_model, "resolved_model"),
            (&self.started_at, "started_at"),
            (&self.completed_at, "completed_at"),
        ] {
            validate_nonempty(value, name)?;
        }
        validate_utc_timestamp(&self.started_at, "started_at")?;
        validate_utc_timestamp(&self.completed_at, "completed_at")?;
        if self.completed_at < self.started_at {
            return Err(anyhow!("invocation completion precedes start"));
        }
        for (value, name) in [
            (&self.candidate_sha256, "candidate_sha256"),
            (
                &self.evaluator_inventory_sha256,
                "evaluator_inventory_sha256",
            ),
            (&self.prompt_sha256, "prompt_sha256"),
            (
                &self.model_visible_context_sha256,
                "model_visible_context_sha256",
            ),
        ] {
            validate_hash(value, name)?;
        }
        if self.input_artifacts.len() > MAX_MODEL_VISIBLE_INPUTS {
            return Err(anyhow!("too many model-visible inputs"));
        }
        validate_unique(
            self.input_artifacts.iter().map(|item| item.name.as_str()),
            "model-visible input name",
        )?;
        for input in &self.input_artifacts {
            validate_nonempty(&input.name, "input name")?;
            validate_hash(&input.sha256, "input sha256")?;
        }
        if self.freshness.resumed {
            return Err(anyhow!(
                "resumed sessions cannot claim fresh-trial evidence"
            ));
        }
        if self.freshness.provenance == EvidenceProvenance::VerifierRecomputed {
            validate_hash(
                self.freshness
                    .verifier_receipt_sha256
                    .as_deref()
                    .ok_or_else(|| anyhow!("verified freshness requires a verifier receipt"))?,
                "freshness verifier receipt",
            )?;
        }
        validate_unique(
            self.isolation.iter().map(|item| item.dimension.as_str()),
            "isolation dimension",
        )?;
        for observation in &self.isolation {
            validate_nonempty(&observation.dimension, "isolation dimension")?;
            if matches!(observation.state, AssuranceEvidenceState::Verified) {
                if observation.provenance != EvidenceProvenance::VerifierRecomputed {
                    return Err(anyhow!(
                        "verified isolation requires verifier-recomputed provenance"
                    ));
                }
                validate_hash(
                    observation
                        .verifier_receipt_sha256
                        .as_deref()
                        .ok_or_else(|| anyhow!("verified isolation requires verifier receipt"))?,
                    "isolation verifier receipt",
                )?;
            }
            if matches!(observation.state, AssuranceEvidenceState::Enforced)
                && matches!(
                    observation.provenance,
                    EvidenceProvenance::CustomerAttested
                        | EvidenceProvenance::HostAttested
                        | EvidenceProvenance::Unknown
                )
            {
                return Err(anyhow!("attestation cannot elevate isolation to enforced"));
            }
        }
        match (&self.terminal_state, &self.output) {
            (TerminalState::Success, Some(output)) => {
                validate_hash(&output.sha256, "output sha256")?;
                validate_hash(&output.lifecycle_policy_sha256, "output lifecycle policy")?;
                if output.byte_count == 0
                    || output.byte_count as usize > MAX_CONFORMANCE_AUTHORITY_BYTES
                {
                    return Err(anyhow!("output byte_count is outside limits"));
                }
            }
            (TerminalState::Success, None) => {
                return Err(anyhow!("successful invocation requires output"));
            }
            (_, Some(_)) => {
                return Err(anyhow!("no-draft invocation cannot expose a usable output"));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluatorChallenge {
    pub(crate) challenge_id: String,
    pub(crate) fixture_id: String,
    pub(crate) job_id: String,
    pub(crate) phase: InvocationPhase,
    pub(crate) expected_terminal_state: TerminalState,
    pub(crate) protected: bool,
    pub(crate) frozen_before_trials: bool,
    pub(crate) model_visible: bool,
    pub(crate) selection_method: String,
    pub(crate) selection_version: String,
    pub(crate) created_at: String,
    pub(crate) frozen_candidate_sha256: String,
    pub(crate) selection_receipt_sha256: String,
    pub(crate) prior_exposure: PriorExposure,
    pub(crate) pack_authored: bool,
    pub(crate) reuse_allowed: bool,
    pub(crate) trial_slots: Vec<EvaluatorTrialSlot>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluatorTrialSlot {
    pub(crate) trial_id: String,
    pub(crate) phase: InvocationPhase,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: String,
    pub(crate) prompt_sha256: String,
    pub(crate) input_artifacts: Vec<ModelVisibleInput>,
    pub(crate) model_visible_context_sha256: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrustedVerifier {
    pub(crate) verifier_name: String,
    pub(crate) verifier_version: String,
    pub(crate) verifier_config_sha256: String,
    pub(crate) identity_authority_sha256: String,
    pub(crate) public_key_hex: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrustedPublicationAuthority {
    pub(crate) reviewer_role: String,
    pub(crate) identity_authority_sha256: String,
    pub(crate) public_key_hex: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PriorExposure {
    NeverExposed,
    Exposed,
    Unknown,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AssertionKind {
    HardBoundary,
    UsefulCompletion,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluatorAssertion {
    pub(crate) assertion_id: String,
    pub(crate) kind: AssertionKind,
    pub(crate) required_trials: u8,
    pub(crate) minimum_passes: u8,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluatorInventoryV1 {
    pub(crate) contract: String,
    pub(crate) evaluator_id: String,
    pub(crate) evaluator_version: String,
    pub(crate) fixture_set_id: String,
    pub(crate) frozen_at: String,
    pub(crate) inventory_sha256: String,
    pub(crate) trusted_verifiers: Vec<TrustedVerifier>,
    pub(crate) trusted_publication_authorities: Vec<TrustedPublicationAuthority>,
    pub(crate) challenges: Vec<EvaluatorChallenge>,
    pub(crate) assertions: Vec<EvaluatorAssertion>,
}
impl ConformanceContract for EvaluatorInventoryV1 {
    const CONTRACT: &'static str = EVALUATOR_INVENTORY_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (v, n) in [
            (&self.evaluator_id, "evaluator_id"),
            (&self.evaluator_version, "evaluator_version"),
            (&self.fixture_set_id, "fixture_set_id"),
            (&self.frozen_at, "frozen_at"),
        ] {
            validate_nonempty(v, n)?;
        }
        validate_utc_timestamp(&self.frozen_at, "inventory frozen_at")?;
        if self.trusted_verifiers.is_empty() || self.trusted_publication_authorities.is_empty() {
            return Err(anyhow!(
                "evaluator inventory requires frozen verifier and publication trust anchors"
            ));
        }
        for verifier in &self.trusted_verifiers {
            validate_nonempty(&verifier.verifier_name, "trusted verifier name")?;
            validate_nonempty(&verifier.verifier_version, "trusted verifier version")?;
            validate_hash(&verifier.verifier_config_sha256, "trusted verifier config")?;
            validate_hash(
                &verifier.identity_authority_sha256,
                "trusted verifier identity authority",
            )?;
            validate_public_key_identity(
                &verifier.public_key_hex,
                &verifier.identity_authority_sha256,
            )?;
        }
        for authority in &self.trusted_publication_authorities {
            validate_nonempty(
                &authority.reviewer_role,
                "trusted publication reviewer role",
            )?;
            validate_hash(
                &authority.identity_authority_sha256,
                "trusted publication identity authority",
            )?;
            validate_public_key_identity(
                &authority.public_key_hex,
                &authority.identity_authority_sha256,
            )?;
        }
        if self.challenges.is_empty() || self.assertions.is_empty() {
            return Err(anyhow!(
                "evaluator inventory requires challenges and assertions"
            ));
        }
        validate_unique(
            self.challenges.iter().map(|v| v.challenge_id.as_str()),
            "challenge id",
        )?;
        validate_unique(
            self.assertions.iter().map(|v| v.assertion_id.as_str()),
            "assertion id",
        )?;
        if self.assertions.len() != 9
            || self
                .assertions
                .iter()
                .map(|assertion| assertion.assertion_id.as_str())
                .ne(["B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B9"])
        {
            return Err(anyhow!("evaluator inventory must define ordered B1-B9"));
        }
        for challenge in &self.challenges {
            if !challenge.protected || !challenge.frozen_before_trials || challenge.model_visible {
                return Err(anyhow!(
                    "challenge must be protected, pre-frozen, and excluded from model context"
                ));
            }
            for (value, name) in [
                (&challenge.selection_method, "challenge selection method"),
                (&challenge.selection_version, "challenge selection version"),
                (&challenge.created_at, "challenge creation time"),
            ] {
                validate_nonempty(value, name)?;
            }
            validate_hash(
                &challenge.frozen_candidate_sha256,
                "challenge frozen candidate digest",
            )?;
            validate_utc_timestamp(&challenge.created_at, "challenge created_at")?;
            validate_hash(
                &challenge.selection_receipt_sha256,
                "challenge selection receipt",
            )?;
            if challenge.prior_exposure != PriorExposure::NeverExposed
                || challenge.pack_authored
                || !challenge.reuse_allowed
            {
                return Err(anyhow!(
                    "challenge independence or declared sampling reuse is not proven"
                ));
            }
            if challenge.trial_slots.len() != REQUIRED_COLD_TRIALS {
                return Err(anyhow!(
                    "challenge requires exactly three frozen trial slots"
                ));
            }
            validate_unique(
                challenge
                    .trial_slots
                    .iter()
                    .map(|slot| slot.trial_id.as_str()),
                "trial slot id",
            )?;
            for slot in &challenge.trial_slots {
                validate_nonempty(&slot.trial_id, "trial slot id")?;
                validate_nonempty(&slot.requested_model, "trial slot requested model")?;
                validate_nonempty(&slot.resolved_model, "trial slot resolved model")?;
                validate_hash(&slot.prompt_sha256, "trial slot prompt sha256")?;
                validate_hash(
                    &slot.model_visible_context_sha256,
                    "trial slot model-visible context sha256",
                )?;
                if slot.phase != challenge.phase
                    || slot.input_artifacts.len() > MAX_MODEL_VISIBLE_INPUTS
                {
                    return Err(anyhow!("trial slot phase or input inventory is invalid"));
                }
                for input in &slot.input_artifacts {
                    validate_nonempty(&input.name, "trial slot input name")?;
                    validate_hash(&input.sha256, "trial slot input sha256")?;
                }
                let digest = canonical_json_sha256_for_domain(
                    "mdp.model-visible-context.v1",
                    &serde_json::to_value(&slot.input_artifacts)?,
                )?;
                if digest != slot.model_visible_context_sha256 {
                    return Err(anyhow!("trial slot model-visible context digest mismatch"));
                }
            }
        }
        for assertion in &self.assertions {
            let useful = assertion.assertion_id == "B6";
            if assertion.required_trials != 3
                || assertion.kind
                    != if useful {
                        AssertionKind::UsefulCompletion
                    } else {
                        AssertionKind::HardBoundary
                    }
                || assertion.minimum_passes != if useful { 2 } else { 3 }
            {
                return Err(anyhow!("invalid evaluator sampling threshold"));
            }
        }
        validate_hash(&self.inventory_sha256, "inventory_sha256")?;
        let mut unhashed = self.clone();
        unhashed.inventory_sha256.clear();
        let digest =
            canonical_json_sha256_for_domain(Self::CONTRACT, &serde_json::to_value(unhashed)?)?;
        if digest != self.inventory_sha256 {
            return Err(anyhow!("evaluator inventory digest mismatch"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScorerType {
    NamedHuman,
    HostEvaluator,
    DeterministicEvaluator,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScorerIdentity {
    pub(crate) scorer_type: ScorerType,
    pub(crate) scorer_id: String,
    pub(crate) reviewer_role: String,
    pub(crate) identity_authority_ref: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScoreStatus {
    Pass,
    Fail,
    Disputed,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssertionScore {
    pub(crate) assertion_id: String,
    pub(crate) status: ScoreStatus,
    pub(crate) rationale: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DisagreementState {
    None,
    Open,
    Resolved,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Adjudication {
    pub(crate) adjudicator_name: String,
    pub(crate) reviewer_role: String,
    pub(crate) identity_authority_ref: String,
    pub(crate) approval_receipt_sha256: String,
    pub(crate) output_sha256: String,
    pub(crate) competing_score_sha256s: Vec<String>,
    pub(crate) decision: ScoreStatus,
    pub(crate) purpose: String,
    pub(crate) approved_at: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluatorResultV1 {
    pub(crate) contract: String,
    pub(crate) result_id: String,
    pub(crate) trial_id: String,
    pub(crate) output_sha256: String,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) evaluator_id: String,
    pub(crate) evaluator_version: String,
    pub(crate) scorer: ScorerIdentity,
    pub(crate) scores: Vec<AssertionScore>,
    pub(crate) competing_score_sha256s: Vec<String>,
    pub(crate) disagreement: DisagreementState,
    pub(crate) adjudication: Option<Adjudication>,
}
impl ConformanceContract for EvaluatorResultV1 {
    const CONTRACT: &'static str = EVALUATOR_RESULT_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (v, n) in [
            (&self.result_id, "result_id"),
            (&self.trial_id, "trial_id"),
            (&self.evaluator_id, "evaluator_id"),
            (&self.evaluator_version, "evaluator_version"),
            (&self.scorer.scorer_id, "scorer_id"),
            (&self.scorer.reviewer_role, "reviewer_role"),
        ] {
            validate_nonempty(v, n)?;
        }
        validate_hash(&self.output_sha256, "output_sha256")?;
        validate_hash(
            &self.evaluator_inventory_sha256,
            "evaluator_inventory_sha256",
        )?;
        if self.scores.is_empty() {
            return Err(anyhow!("evaluator result requires scores"));
        }
        validate_unique(
            self.scores.iter().map(|s| s.assertion_id.as_str()),
            "assertion score",
        )?;
        for s in &self.scores {
            validate_nonempty(&s.rationale, "score rationale")?;
        }
        match self.disagreement {
            DisagreementState::None
                if self.adjudication.is_some() || !self.competing_score_sha256s.is_empty() =>
            {
                return Err(anyhow!("non-disputed result cannot include adjudication"));
            }
            DisagreementState::Open => {
                return Err(anyhow!("open disagreement cannot satisfy evaluator result"));
            }
            DisagreementState::Resolved => {
                let a = self
                    .adjudication
                    .as_ref()
                    .ok_or_else(|| anyhow!("resolved disagreement requires adjudication"))?;
                if a.identity_authority_ref.is_empty() || a.adjudicator_name.is_empty() {
                    return Err(anyhow!(
                        "adjudication requires named human and identity authority"
                    ));
                }
                validate_hash(&a.approval_receipt_sha256, "adjudication approval receipt")?;
                if a.reviewer_role != "independent-customer-adjudicator"
                    || a.purpose != "resolve-hard-boundary"
                    || a.decision == ScoreStatus::Disputed
                    || a.competing_score_sha256s.len() < 2
                {
                    return Err(anyhow!(
                        "adjudication requires an independent customer adjudicator and final decision"
                    ));
                }
                if !valid_date_time(&a.approved_at) {
                    return Err(anyhow!(
                        "adjudication approved_at must be an RFC 3339 date-time"
                    ));
                }
                if a.output_sha256 != self.output_sha256
                    || a.competing_score_sha256s != self.competing_score_sha256s
                {
                    return Err(anyhow!("adjudication bindings mismatch"));
                }
                for h in &a.competing_score_sha256s {
                    validate_hash(h, "competing score hash")?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AccessClass {
    Private,
    Synthetic,
    SanitizedPublic,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeletionDisposition {
    Delete,
    Archive,
    ReviewRequired,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CapabilityStatus {
    Supported,
    Unsupported,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LifecycleCapabilities {
    pub(crate) access: CapabilityStatus,
    pub(crate) retention: CapabilityStatus,
    pub(crate) deletion: CapabilityStatus,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrivateRecordPolicyV1 {
    pub(crate) contract: String,
    pub(crate) policy_id: String,
    pub(crate) access_class: AccessClass,
    pub(crate) policy_owner_or_ref: String,
    pub(crate) retention_until: String,
    pub(crate) deletion_disposition: DeletionDisposition,
    pub(crate) host_capabilities: LifecycleCapabilities,
}
impl ConformanceContract for PrivateRecordPolicyV1 {
    const CONTRACT: &'static str = PRIVATE_RECORD_POLICY_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (v, n) in [
            (&self.policy_id, "policy_id"),
            (&self.policy_owner_or_ref, "policy_owner_or_ref"),
            (&self.retention_until, "retention_until"),
        ] {
            validate_nonempty(v, n)?;
        }
        validate_utc_timestamp(&self.retention_until, "retention_until")?;
        if self.host_capabilities.access != CapabilityStatus::Supported
            || self.host_capabilities.retention != CapabilityStatus::Supported
            || self.host_capabilities.deletion != CapabilityStatus::Supported
        {
            return Err(anyhow!("no-draft:policy-blocked"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicationApprovalV1 {
    pub(crate) contract: String,
    pub(crate) approval_id: String,
    pub(crate) artifact_sha256: String,
    pub(crate) classification: AccessClass,
    pub(crate) approved_by: String,
    pub(crate) reviewer_role: String,
    pub(crate) identity_authority_sha256: String,
    pub(crate) approved_at: String,
    pub(crate) purpose: String,
    pub(crate) signature_hex: String,
}
impl PublicationApprovalV1 {
    pub(crate) fn approves_exact_hash(&self, digest: &str) -> bool {
        self.classification == AccessClass::SanitizedPublic && self.artifact_sha256 == digest
    }
    pub(crate) fn verify_signature(&self, public_key_hex: &str) -> Result<()> {
        verify_authority_signature(
            self,
            &self.signature_hex,
            public_key_hex,
            "mdp.publication-approval.v1.signature.v1",
            |value| value.signature_hex.clear(),
        )
    }
}
impl ConformanceContract for PublicationApprovalV1 {
    const CONTRACT: &'static str = PUBLICATION_APPROVAL_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        validate_hash(&self.artifact_sha256, "artifact_sha256")?;
        if self.classification != AccessClass::SanitizedPublic {
            return Err(anyhow!("publication approval must be sanitized-public"));
        }
        for (v, n) in [
            (&self.approval_id, "approval_id"),
            (&self.approved_by, "approved_by"),
            (&self.reviewer_role, "reviewer_role"),
            (&self.approved_at, "approved_at"),
            (&self.purpose, "purpose"),
        ] {
            validate_nonempty(v, n)?;
        }
        validate_hash(&self.identity_authority_sha256, "identity_authority_sha256")?;
        validate_hex(&self.signature_hex, 128, "signature_hex")?;
        validate_utc_timestamp(&self.approved_at, "approved_at")?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BehavioralStatus {
    Unassessed,
    Passed,
    Failed,
    Malformed,
    BoundedNonSuccessConfirmed,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceTrialV1 {
    pub(crate) contract: String,
    pub(crate) trial_id: String,
    pub(crate) candidate_sha256: String,
    pub(crate) invocation_sha256: String,
    pub(crate) evaluator_result_sha256s: Vec<String>,
    pub(crate) terminal_state: TerminalState,
    pub(crate) useful_completion: Option<bool>,
    pub(crate) expected_bounded_non_success: bool,
    pub(crate) lifecycle_policy_sha256: String,
    pub(crate) publication_approval_sha256s: Vec<String>,
}
impl ConformanceContract for ConformanceTrialV1 {
    const CONTRACT: &'static str = CONFORMANCE_TRIAL_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        validate_nonempty(&self.trial_id, "trial_id")?;
        for h in std::iter::once(&self.candidate_sha256)
            .chain(std::iter::once(&self.invocation_sha256))
            .chain(self.evaluator_result_sha256s.iter())
            .chain(std::iter::once(&self.lifecycle_policy_sha256))
            .chain(self.publication_approval_sha256s.iter())
        {
            validate_hash(h, "trial authority hash")?;
        }
        if self.terminal_state == TerminalState::Success && self.expected_bounded_non_success {
            return Err(anyhow!("successful trial cannot be expected non-success"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeterministicStatus {
    Unassessed,
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeterministicVerdict {
    SufficientForJob,
    NotSufficientForJob,
    Unassessed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeterministicEvaluatorIdentity {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) fixture_set_id: String,
    pub(crate) inventory_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeterministicAuthorityRef {
    pub(crate) role: CandidateAuthorityRole,
    pub(crate) contract: String,
    pub(crate) relative_path: String,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeterministicAssertion {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) scope: String,
    pub(crate) hard: bool,
    pub(crate) status: String,
    pub(crate) authority_refs: Vec<DeterministicAuthorityRef>,
    pub(crate) reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeterministicSummary {
    pub(crate) passed: usize,
    pub(crate) failed: usize,
    pub(crate) unassessed: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeterministicConformanceV1 {
    pub(crate) contract: String,
    pub(crate) valid: bool,
    pub(crate) candidate_id: String,
    pub(crate) job_id: String,
    pub(crate) pack_release: PackReleaseIdentity,
    pub(crate) evaluator: DeterministicEvaluatorIdentity,
    pub(crate) fixture_id: String,
    pub(crate) challenge_id: Option<String>,
    pub(crate) status: DeterministicVerdict,
    pub(crate) behavioral_qualification_allowed: bool,
    pub(crate) assertions: Vec<DeterministicAssertion>,
    pub(crate) summary: DeterministicSummary,
}

impl DeterministicConformanceV1 {
    pub(crate) fn derived_status(&self) -> DeterministicStatus {
        match self.status {
            DeterministicVerdict::SufficientForJob => DeterministicStatus::Passed,
            DeterministicVerdict::NotSufficientForJob => DeterministicStatus::Failed,
            DeterministicVerdict::Unassessed => DeterministicStatus::Unassessed,
        }
    }
}

impl ConformanceContract for DeterministicConformanceV1 {
    const CONTRACT: &'static str = DETERMINISTIC_CONFORMANCE_V1;

    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (value, field) in [
            (&self.candidate_id, "candidate_id"),
            (&self.job_id, "job_id"),
            (&self.fixture_id, "fixture_id"),
            (&self.evaluator.id, "evaluator id"),
            (&self.evaluator.version, "evaluator version"),
            (&self.evaluator.fixture_set_id, "fixture_set_id"),
        ] {
            validate_nonempty(value, field)?;
        }
        validate_hash(&self.evaluator.inventory_sha256, "inventory_sha256")?;
        if self.assertions.len() != 12 {
            return Err(anyhow!("deterministic conformance requires exactly D1-D12"));
        }
        let expected = (1..=12).map(|n| format!("D{n}")).collect::<Vec<_>>();
        if self.assertions.iter().map(|a| &a.id).ne(expected.iter()) {
            return Err(anyhow!(
                "deterministic assertions must be ordered exactly D1-D12"
            ));
        }
        let mut passed = 0;
        let mut failed = 0;
        let mut unassessed = 0;
        for assertion in &self.assertions {
            if !assertion.hard || !matches!(assertion.scope.as_str(), "release" | "fixture") {
                return Err(anyhow!("deterministic assertion policy is invalid"));
            }
            match assertion.status.as_str() {
                "pass" => passed += 1,
                "fail" => failed += 1,
                "unassessed" => unassessed += 1,
                _ => return Err(anyhow!("invalid deterministic assertion status")),
            }
            for authority in &assertion.authority_refs {
                validate_nonempty(&authority.contract, "authority contract")?;
                validate_relative_path(&authority.relative_path)?;
                validate_hash(&authority.sha256, "authority sha256")?;
            }
        }
        if (passed, failed, unassessed)
            != (
                self.summary.passed,
                self.summary.failed,
                self.summary.unassessed,
            )
        {
            return Err(anyhow!("deterministic assertion summary mismatch"));
        }
        let derived = if failed > 0 {
            DeterministicVerdict::NotSufficientForJob
        } else if unassessed > 0 {
            DeterministicVerdict::Unassessed
        } else {
            DeterministicVerdict::SufficientForJob
        };
        if self.status != derived
            || self.valid != (derived == DeterministicVerdict::SufficientForJob)
            || self.behavioral_qualification_allowed
                != (derived == DeterministicVerdict::SufficientForJob)
        {
            return Err(anyhow!("deterministic verdict contradicts assertions"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum QualificationVerdict {
    #[serde(rename = "qualified-for-job-under-envelope")]
    QualifiedForJobUnderEnvelope,
    #[serde(rename = "not-qualified-for-job-under-envelope")]
    NotQualifiedForJobUnderEnvelope,
    #[serde(rename = "not-sufficient-for-job")]
    NotSufficientForJob,
    #[serde(rename = "unassessed")]
    Unassessed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AssertionEvaluationStatus {
    Passed,
    Failed,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JobSufficiency {
    SufficientForJob,
    NotSufficientForJob,
    Unassessed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum BehavioralQualification {
    #[serde(rename = "qualified-for-job-under-envelope")]
    QualifiedForJobUnderEnvelope,
    #[serde(rename = "not-qualified-for-job-under-envelope")]
    NotQualifiedForJobUnderEnvelope,
    #[serde(rename = "unassessed")]
    Unassessed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceAssertionEvaluation {
    pub(crate) id: String,
    pub(crate) status: AssertionEvaluationStatus,
    pub(crate) passed_trials: u8,
    pub(crate) required_trials: u8,
    pub(crate) reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BehavioralTrialEvaluation {
    pub(crate) trial_id: String,
    pub(crate) status: BehavioralStatus,
    pub(crate) usable_output: bool,
    pub(crate) reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BehavioralEvaluation {
    pub(crate) contract: String,
    pub(crate) valid: bool,
    pub(crate) job_id: String,
    pub(crate) candidate_sha256: String,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) lifecycle_policy_sha256: String,
    pub(crate) deterministic_evaluation_sha256: String,
    pub(crate) trial_sha256s: Vec<String>,
    pub(crate) deterministic_status: DeterministicStatus,
    pub(crate) job_sufficiency: JobSufficiency,
    pub(crate) preflight_assertions: Vec<ConformanceAssertionEvaluation>,
    pub(crate) behavioral_assertions: Vec<ConformanceAssertionEvaluation>,
    pub(crate) trials: Vec<BehavioralTrialEvaluation>,
    pub(crate) behavioral_qualification: BehavioralQualification,
    pub(crate) overall_result: QualificationVerdict,
    pub(crate) drafting_authority_granted: bool,
    pub(crate) reason_codes: Vec<String>,
}
impl ConformanceContract for BehavioralEvaluation {
    const CONTRACT: &'static str = BEHAVIORAL_EVALUATION_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        validate_nonempty(&self.job_id, "job_id")?;
        for (hash, name) in [
            (&self.candidate_sha256, "candidate_sha256"),
            (
                &self.evaluator_inventory_sha256,
                "evaluator_inventory_sha256",
            ),
            (&self.lifecycle_policy_sha256, "lifecycle_policy_sha256"),
            (
                &self.deterministic_evaluation_sha256,
                "deterministic_evaluation_sha256",
            ),
        ] {
            validate_hash(hash, name)?;
        }
        for hash in &self.trial_sha256s {
            validate_hash(hash, "trial_sha256")?;
        }
        validate_unique(self.trial_sha256s.iter().map(String::as_str), "trial hash")?;
        if self.trial_sha256s.len() != self.trials.len() {
            return Err(anyhow!(
                "behavioral evaluation trial bindings are incomplete"
            ));
        }
        if self.preflight_assertions.len() != 4
            || self.trials.len() > MAX_TRIALS_PER_JOB
            || self.drafting_authority_granted
        {
            return Err(anyhow!(
                "behavioral evaluation has invalid assertion, trial, or drafting-authority shape"
            ));
        }
        if self
            .preflight_assertions
            .iter()
            .map(|item| item.id.as_str())
            .ne(["Q1", "Q2", "Q3", "Q4"])
        {
            return Err(anyhow!(
                "preflight assertions must be ordered exactly Q1-Q4"
            ));
        }
        validate_unique(
            self.preflight_assertions
                .iter()
                .map(|item| item.id.as_str()),
            "preflight assertion id",
        )?;
        validate_unique(
            self.behavioral_assertions
                .iter()
                .map(|item| item.id.as_str()),
            "behavioral assertion id",
        )?;
        if self.behavioral_assertions.len() != 9
            || self
                .behavioral_assertions
                .iter()
                .map(|item| item.id.as_str())
                .ne(["B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B9"])
        {
            return Err(anyhow!(
                "behavioral assertions must be ordered exactly B1-B9"
            ));
        }
        validate_unique(
            self.trials.iter().map(|item| item.trial_id.as_str()),
            "trial id",
        )?;
        for assertion in &self.preflight_assertions {
            if assertion.required_trials != REQUIRED_COLD_TRIALS as u8
                || assertion.passed_trials > assertion.required_trials
                || assertion.status == AssertionEvaluationStatus::Passed
                    && (assertion.passed_trials != assertion.required_trials
                        || !assertion.reason_codes.is_empty())
                || assertion.status == AssertionEvaluationStatus::Failed
                    && assertion.reason_codes.is_empty()
            {
                return Err(anyhow!(
                    "preflight assertion result contradicts frozen sampling"
                ));
            }
        }
        for assertion in &self.behavioral_assertions {
            let minimum = if assertion.id == "B6" { 2 } else { 3 };
            if assertion.required_trials != REQUIRED_COLD_TRIALS as u8
                || assertion.passed_trials > assertion.required_trials
                || assertion.status == AssertionEvaluationStatus::Passed
                    && (assertion.passed_trials < minimum || !assertion.reason_codes.is_empty())
                || assertion.status == AssertionEvaluationStatus::Failed
                    && (assertion.passed_trials >= minimum || assertion.reason_codes.is_empty())
            {
                return Err(anyhow!(
                    "behavioral assertion result contradicts closed threshold policy"
                ));
            }
        }
        let complete_sampling = self.trials.len() == REQUIRED_COLD_TRIALS
            && self.trial_sha256s.len() == REQUIRED_COLD_TRIALS;
        let any_failed = self
            .preflight_assertions
            .iter()
            .chain(self.behavioral_assertions.iter())
            .any(|item| item.status == AssertionEvaluationStatus::Failed);
        let any_unassessed = self
            .preflight_assertions
            .iter()
            .chain(self.behavioral_assertions.iter())
            .any(|item| item.status == AssertionEvaluationStatus::NotApplicable);
        let expected_sufficiency = match self.deterministic_status {
            DeterministicStatus::Passed => JobSufficiency::SufficientForJob,
            DeterministicStatus::Failed => JobSufficiency::NotSufficientForJob,
            DeterministicStatus::Unassessed => JobSufficiency::Unassessed,
        };
        let malformed_trial = self
            .trials
            .iter()
            .any(|trial| trial.status == BehavioralStatus::Malformed);
        let failed_trials = self
            .trials
            .iter()
            .filter(|trial| trial.status == BehavioralStatus::Failed)
            .count();
        let useful_misses = self
            .behavioral_assertions
            .iter()
            .find(|assertion| assertion.id == "B6")
            .map(|assertion| {
                assertion
                    .required_trials
                    .saturating_sub(assertion.passed_trials) as usize
            })
            .unwrap_or_default();
        let expected_qualification = if self.deterministic_status != DeterministicStatus::Passed {
            BehavioralQualification::Unassessed
        } else if any_failed || malformed_trial {
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        } else if !complete_sampling || any_unassessed {
            BehavioralQualification::Unassessed
        } else {
            BehavioralQualification::QualifiedForJobUnderEnvelope
        };
        let expected_overall = match (expected_sufficiency, expected_qualification) {
            (JobSufficiency::NotSufficientForJob, _) => QualificationVerdict::NotSufficientForJob,
            (
                JobSufficiency::SufficientForJob,
                BehavioralQualification::QualifiedForJobUnderEnvelope,
            ) => QualificationVerdict::QualifiedForJobUnderEnvelope,
            (
                JobSufficiency::SufficientForJob,
                BehavioralQualification::NotQualifiedForJobUnderEnvelope,
            ) => QualificationVerdict::NotQualifiedForJobUnderEnvelope,
            _ => QualificationVerdict::Unassessed,
        };
        if expected_qualification == BehavioralQualification::QualifiedForJobUnderEnvelope
            && failed_trials > useful_misses
        {
            return Err(anyhow!(
                "qualified behavioral trial statuses contradict usefulness sampling"
            ));
        }
        if self.job_sufficiency != expected_sufficiency
            || self.behavioral_qualification != expected_qualification
            || self.overall_result != expected_overall
            || self.valid
                != (expected_qualification == BehavioralQualification::QualifiedForJobUnderEnvelope)
        {
            return Err(anyhow!(
                "behavioral evaluation aggregate contradicts assertion results"
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JourneyPhase {
    Candidate,
    Normalization,
    Selection,
    Generation,
    Review,
    DeterministicEvaluation,
    BehavioralEvaluation,
    Publication,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JourneyArtifactRole {
    Candidate,
    PackRelease,
    Requirements,
    ProductFoundation,
    SkillsRoute,
    Prompt,
    PromptInvocation,
    SourceLineage,
    NormalizedInput,
    RoutedContext,
    GovernedOutput,
    ClaimsValidation,
    DecisionResult,
    RunBundle,
    RunReceipt,
    RunVerification,
    EvaluatorInventory,
    PrivateRecordPolicy,
    PublicationApproval,
    DeterministicEvaluation,
    BehavioralEvaluation,
    Trial,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JourneyRelation {
    Declares,
    Normalizes,
    Selects,
    Generates,
    Reviews,
    Evaluates,
    Verifies,
    BoundTo,
    Blocks,
    Approves,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JourneyArtifact {
    pub(crate) artifact_id: String,
    pub(crate) phase: JourneyPhase,
    pub(crate) role: JourneyArtifactRole,
    pub(crate) contract: String,
    pub(crate) relative_path: Option<String>,
    pub(crate) opaque_artifact_id: Option<String>,
    pub(crate) authority_sha256: String,
    pub(crate) byte_count: Option<u64>,
    pub(crate) access_class: AccessClass,
    pub(crate) publication_approval_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JourneyLink {
    pub(crate) from_artifact_id: String,
    pub(crate) to_artifact_id: String,
    pub(crate) relation: JourneyRelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceJourney {
    pub(crate) subject_class: String,
    pub(crate) synthetic_subject: bool,
    pub(crate) artifacts: Vec<JourneyArtifact>,
    pub(crate) links: Vec<JourneyLink>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobConformanceV1 {
    pub(crate) contract: String,
    pub(crate) candidate_id: String,
    pub(crate) job_id: String,
    pub(crate) fixture_id: String,
    pub(crate) pack_release: PackReleaseIdentity,
    pub(crate) candidate_sha256: String,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) lifecycle_policy_sha256: String,
    pub(crate) deterministic_evaluation_sha256: String,
    pub(crate) behavioral_evaluation_sha256: String,
    pub(crate) deterministic_status: DeterministicStatus,
    pub(crate) behavioral_status: BehavioralStatus,
    pub(crate) verdict: QualificationVerdict,
    pub(crate) trial_sha256s: Vec<String>,
    pub(crate) journey: ConformanceJourney,
    pub(crate) limitations: Vec<String>,
}
impl ConformanceContract for JobConformanceV1 {
    const CONTRACT: &'static str = JOB_CONFORMANCE_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (value, name) in [
            (&self.candidate_id, "candidate_id"),
            (&self.job_id, "job_id"),
            (&self.fixture_id, "fixture_id"),
        ] {
            validate_nonempty(value, name)?;
        }
        validate_hash(&self.candidate_sha256, "candidate_sha256")?;
        validate_hash(
            &self.evaluator_inventory_sha256,
            "evaluator_inventory_sha256",
        )?;
        for (hash, name) in [
            (&self.lifecycle_policy_sha256, "lifecycle_policy_sha256"),
            (
                &self.deterministic_evaluation_sha256,
                "deterministic_evaluation_sha256",
            ),
            (
                &self.behavioral_evaluation_sha256,
                "behavioral_evaluation_sha256",
            ),
            (&self.pack_release.portable_digest, "portable_digest"),
            (&self.pack_release.source_revision, "source_revision"),
        ] {
            validate_hash(hash, name)?;
        }
        validate_nonempty(&self.journey.subject_class, "journey subject_class")?;
        if !self.journey.synthetic_subject {
            return Err(anyhow!("v1 composite journey requires a synthetic subject"));
        }
        if self.trial_sha256s.len() > MAX_TRIALS_PER_JOB
            || self.journey.artifacts.len() > MAX_JOURNEY_LINKS
            || self.journey.links.len() > MAX_JOURNEY_LINKS
        {
            return Err(anyhow!("job conformance fan-out exceeds limit"));
        }
        for h in &self.trial_sha256s {
            validate_hash(h, "trial sha256")?;
        }
        for limitation in &self.limitations {
            validate_public_reason_code(limitation)?;
        }
        validate_unique(self.trial_sha256s.iter().map(String::as_str), "trial hash")?;
        validate_unique(
            self.journey
                .artifacts
                .iter()
                .map(|item| item.artifact_id.as_str()),
            "journey artifact id",
        )?;
        let artifact_ids = self
            .journey
            .artifacts
            .iter()
            .map(|item| item.artifact_id.as_str())
            .collect::<HashSet<_>>();
        for artifact in &self.journey.artifacts {
            validate_nonempty(&artifact.artifact_id, "journey artifact id")?;
            validate_nonempty(&artifact.contract, "journey artifact contract")?;
            validate_hash(&artifact.authority_sha256, "journey authority hash")?;
            match (&artifact.relative_path, &artifact.opaque_artifact_id) {
                (Some(path), None) => {
                    validate_relative_path(path)?;
                    if artifact.byte_count.is_none() {
                        return Err(anyhow!("path-backed journey artifact requires byte_count"));
                    }
                }
                (None, Some(id)) => {
                    validate_nonempty(id, "opaque artifact id")?;
                    if artifact.byte_count.is_some() {
                        return Err(anyhow!("opaque journey artifact cannot claim byte_count"));
                    }
                    if matches!(
                        artifact.role,
                        JourneyArtifactRole::Candidate
                            | JourneyArtifactRole::PackRelease
                            | JourneyArtifactRole::Requirements
                            | JourneyArtifactRole::SkillsRoute
                            | JourneyArtifactRole::Prompt
                            | JourneyArtifactRole::EvaluatorInventory
                            | JourneyArtifactRole::PrivateRecordPolicy
                            | JourneyArtifactRole::DeterministicEvaluation
                            | JourneyArtifactRole::BehavioralEvaluation
                    ) {
                        return Err(anyhow!(
                            "opaque reference cannot satisfy deterministic authority"
                        ));
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "journey artifact requires exactly one path or opaque id"
                    ));
                }
            }
            match artifact.access_class {
                AccessClass::Synthetic => {
                    if artifact.publication_approval_sha256.is_some() {
                        return Err(anyhow!(
                            "synthetic artifact cannot use publication approval"
                        ));
                    }
                }
                AccessClass::SanitizedPublic => validate_hash(
                    artifact
                        .publication_approval_sha256
                        .as_deref()
                        .ok_or_else(|| {
                            anyhow!("sanitized-public artifact requires exact-hash approval")
                        })?,
                    "publication approval hash",
                )?,
                AccessClass::Private => {
                    if artifact.publication_approval_sha256.is_some() {
                        return Err(anyhow!("private artifact cannot use publication approval"));
                    }
                }
            }
        }
        for link in &self.journey.links {
            if link.from_artifact_id == link.to_artifact_id
                || !artifact_ids.contains(link.from_artifact_id.as_str())
                || !artifact_ids.contains(link.to_artifact_id.as_str())
            {
                return Err(anyhow!("journey link is dangling or self-referential"));
            }
        }
        let mut indegree = self
            .journey
            .artifacts
            .iter()
            .map(|artifact| (artifact.artifact_id.as_str(), 0usize))
            .collect::<HashMap<_, _>>();
        let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
        for link in &self.journey.links {
            *indegree
                .get_mut(link.to_artifact_id.as_str())
                .expect("link endpoints were validated") += 1;
            outgoing
                .entry(link.from_artifact_id.as_str())
                .or_default()
                .push(link.to_artifact_id.as_str());
        }
        let mut ready = indegree
            .iter()
            .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
            .collect::<Vec<_>>();
        let mut visited = 0usize;
        while let Some(id) = ready.pop() {
            visited += 1;
            for target in outgoing.get(id).into_iter().flatten() {
                let degree = indegree
                    .get_mut(target)
                    .expect("link endpoints were validated");
                *degree -= 1;
                if *degree == 0 {
                    ready.push(target);
                }
            }
        }
        if visited != self.journey.artifacts.len() {
            return Err(anyhow!("journey links must form an acyclic graph"));
        }
        for required in [
            JourneyArtifactRole::Candidate,
            JourneyArtifactRole::PackRelease,
            JourneyArtifactRole::Requirements,
            JourneyArtifactRole::Prompt,
            JourneyArtifactRole::EvaluatorInventory,
            JourneyArtifactRole::PrivateRecordPolicy,
            JourneyArtifactRole::DeterministicEvaluation,
            JourneyArtifactRole::BehavioralEvaluation,
        ] {
            if !self
                .journey
                .artifacts
                .iter()
                .any(|item| item.role == required)
            {
                return Err(anyhow!(
                    "journey missing required artifact role: {required:?}"
                ));
            }
        }
        for (role, expected) in [
            (
                JourneyArtifactRole::Candidate,
                self.candidate_sha256.as_str(),
            ),
            (
                JourneyArtifactRole::EvaluatorInventory,
                self.evaluator_inventory_sha256.as_str(),
            ),
            (
                JourneyArtifactRole::PrivateRecordPolicy,
                self.lifecycle_policy_sha256.as_str(),
            ),
            (
                JourneyArtifactRole::DeterministicEvaluation,
                self.deterministic_evaluation_sha256.as_str(),
            ),
            (
                JourneyArtifactRole::BehavioralEvaluation,
                self.behavioral_evaluation_sha256.as_str(),
            ),
        ] {
            if !self
                .journey
                .artifacts
                .iter()
                .any(|artifact| artifact.role == role && artifact.authority_sha256 == expected)
            {
                return Err(anyhow!(
                    "journey top-level authority binding mismatch: {role:?}"
                ));
            }
        }
        let journey_trial_hashes = self
            .journey
            .artifacts
            .iter()
            .filter(|artifact| artifact.role == JourneyArtifactRole::Trial)
            .map(|artifact| artifact.authority_sha256.as_str())
            .collect::<Vec<_>>();
        if journey_trial_hashes
            != self
                .trial_sha256s
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        {
            return Err(anyhow!(
                "journey trial bindings do not match top-level trial set"
            ));
        }
        let artifact_for = |role| {
            self.journey
                .artifacts
                .iter()
                .find(|artifact| artifact.role == role)
                .map(|artifact| artifact.artifact_id.as_str())
        };
        let requirements = artifact_for(JourneyArtifactRole::Requirements).unwrap();
        let deterministic = artifact_for(JourneyArtifactRole::DeterministicEvaluation).unwrap();
        let behavioral = artifact_for(JourneyArtifactRole::BehavioralEvaluation).unwrap();
        let has_link = |from: &str, to: &str, relation: JourneyRelation| {
            self.journey.links.iter().any(|link| {
                link.from_artifact_id == from
                    && link.to_artifact_id == to
                    && link.relation == relation
            })
        };
        if !has_link(requirements, deterministic, JourneyRelation::Evaluates)
            || !has_link(deterministic, behavioral, JourneyRelation::BoundTo)
        {
            return Err(anyhow!("journey required evaluation chain is incomplete"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceReportV1 {
    pub(crate) contract: String,
    pub(crate) report_id: String,
    pub(crate) pack_release: PackReleaseIdentity,
    pub(crate) evaluator_inventory_sha256: String,
    pub(crate) job_conformance_sha256s: Vec<String>,
    pub(crate) generated_at: String,
    pub(crate) lifecycle_policy_sha256: String,
}
impl ConformanceContract for ConformanceReportV1 {
    const CONTRACT: &'static str = CONFORMANCE_REPORT_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        validate_nonempty(&self.report_id, "report_id")?;
        validate_utc_timestamp(&self.generated_at, "generated_at")?;
        for h in std::iter::once(&self.evaluator_inventory_sha256)
            .chain(self.job_conformance_sha256s.iter())
            .chain(std::iter::once(&self.lifecycle_policy_sha256))
        {
            validate_hash(h, "report authority hash")?;
        }
        validate_unique(
            self.job_conformance_sha256s.iter().map(String::as_str),
            "job conformance hash",
        )?;
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicEvidenceDigest {
    pub(crate) artifact_role: JourneyArtifactRole,
    pub(crate) artifact_sha256: Option<String>,
    pub(crate) classification: AccessClass,
    pub(crate) publication_approved: bool,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicJobResult {
    pub(crate) job_id: String,
    pub(crate) deterministic_status: DeterministicStatus,
    pub(crate) behavioral_status: BehavioralStatus,
    pub(crate) verdict: QualificationVerdict,
    pub(crate) evidence: Vec<PublicEvidenceDigest>,
    pub(crate) limitations: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicConformanceReportV1 {
    pub(crate) contract: String,
    pub(crate) report_id: String,
    pub(crate) pack_id: String,
    pub(crate) release_id: String,
    pub(crate) evaluator_id: String,
    pub(crate) evaluator_version: String,
    pub(crate) generated_at: String,
    pub(crate) jobs: Vec<PublicJobResult>,
}
impl ConformanceContract for PublicConformanceReportV1 {
    const CONTRACT: &'static str = PUBLIC_CONFORMANCE_REPORT_V1;
    fn validate(&self) -> Result<()> {
        validate_contract(&self.contract, Self::CONTRACT)?;
        for (v, n) in [
            (&self.report_id, "report_id"),
            (&self.pack_id, "pack_id"),
            (&self.release_id, "release_id"),
            (&self.evaluator_id, "evaluator_id"),
            (&self.evaluator_version, "evaluator_version"),
        ] {
            validate_nonempty(v, n)?;
        }
        validate_utc_timestamp(&self.generated_at, "generated_at")?;
        for job in &self.jobs {
            for limitation in &job.limitations {
                validate_public_reason_code(limitation)?;
            }
            for evidence in &job.evidence {
                match evidence.classification {
                    AccessClass::Synthetic => {
                        if let Some(h) = &evidence.artifact_sha256 {
                            validate_hash(h, "synthetic artifact hash")?;
                        }
                        if evidence.publication_approved {
                            return Err(anyhow!(
                                "synthetic evidence does not use publication approval"
                            ));
                        }
                    }
                    AccessClass::SanitizedPublic => {
                        validate_hash(
                            evidence.artifact_sha256.as_deref().ok_or_else(|| {
                                anyhow!("sanitized-public evidence requires artifact hash")
                            })?,
                            "public artifact hash",
                        )?;
                        if !evidence.publication_approved {
                            return Err(anyhow!("sanitized-public digest requires approval"));
                        }
                    }
                    AccessClass::Private => {
                        if evidence.artifact_sha256.is_some() || evidence.publication_approved {
                            return Err(anyhow!(
                                "private public evidence may expose only an opaque local id"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) const BEHAVIORAL_EVALUATION_V1: &str = "mdp.behavioral-evaluation.v1";
const COLD_ISOLATION_DIMENSIONS: [&str; 3] = ["memory", "tools", "neighboring-context"];

/// Validates already-recorded external behavioral evidence and aggregates it.
/// This function never invokes a model and never grants permission to draft.
pub(crate) fn evaluate_behavioral_trials(
    candidate: &ConformanceCandidateV1,
    inventory: &EvaluatorInventoryV1,
    lifecycle: &PrivateRecordPolicyV1,
    deterministic: &DeterministicConformanceV1,
    invocations: &[ModelInvocationEvidenceV1],
    trials: &[ConformanceTrialV1],
    results: &[EvaluatorResultV1],
    approvals: &[PublicationApprovalV1],
    verifier_receipts: &[ConformanceVerifierReceiptV1],
) -> Result<BehavioralEvaluation> {
    candidate.validate()?;
    inventory.validate()?;
    lifecycle.validate()?;
    deterministic.validate()?;
    for invocation in invocations {
        invocation.validate()?;
    }
    for trial in trials {
        trial.validate()?;
    }
    for result in results {
        result.validate()?;
    }
    for approval in approvals {
        approval.validate()?;
    }
    for receipt in verifier_receipts {
        receipt.validate()?;
    }

    let candidate_digest = canonical_authority_sha256(candidate)?;
    if deterministic.candidate_id != candidate.candidate_id
        || deterministic.job_id != candidate.job_id
        || deterministic.fixture_id != candidate.fixture_id
        || deterministic.pack_release != candidate.pack_release
        || deterministic.evaluator.inventory_sha256 != inventory.inventory_sha256
        || deterministic.evaluator.id != inventory.evaluator_id
        || deterministic.evaluator.version != inventory.evaluator_version
        || deterministic.evaluator.fixture_set_id != inventory.fixture_set_id
        || deterministic.challenge_id != candidate.challenge_id
    {
        return Err(anyhow!(
            "deterministic evaluation authority binding mismatch"
        ));
    }
    let deterministic_status = deterministic.derived_status();
    let deterministic_evaluation_sha256 = canonical_authority_sha256(deterministic)?;
    let inventory_digest = inventory.inventory_sha256.as_str();
    let lifecycle_digest = canonical_authority_sha256(lifecycle)?;
    let challenge = inventory
        .challenges
        .iter()
        .find(|challenge| {
            challenge.job_id == candidate.job_id
                && challenge.fixture_id == candidate.fixture_id
                && candidate
                    .challenge_id
                    .as_deref()
                    .is_some_and(|id| id == challenge.challenge_id)
        })
        .ok_or_else(|| anyhow!("candidate challenge is absent from evaluator inventory"))?;

    let invocation_by_trial: std::collections::HashMap<_, _> = invocations
        .iter()
        .map(|invocation| (invocation.trial_id.as_str(), invocation))
        .collect();
    let result_by_hash: std::collections::HashMap<_, _> = results
        .iter()
        .map(|result| Ok((canonical_authority_sha256(result)?, result)))
        .collect::<Result<_>>()?;
    let approval_by_hash: std::collections::HashMap<_, _> = approvals
        .iter()
        .map(|approval| Ok((canonical_authority_sha256(approval)?, approval)))
        .collect::<Result<_>>()?;
    let verifier_by_hash: std::collections::HashMap<_, _> = verifier_receipts
        .iter()
        .map(|receipt| Ok((canonical_authority_sha256(receipt)?, receipt)))
        .collect::<Result<_>>()?;

    let mut reasons = Vec::new();
    let mut q_reasons = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let declared_slots = challenge
        .trial_slots
        .iter()
        .map(|slot| slot.trial_id.as_str())
        .collect::<HashSet<_>>();
    let submitted_trials = trials
        .iter()
        .map(|trial| trial.trial_id.as_str())
        .collect::<HashSet<_>>();
    let submitted_invocations = invocations
        .iter()
        .map(|invocation| invocation.trial_id.as_str())
        .collect::<HashSet<_>>();
    let complete_sampling = trials.len() == REQUIRED_COLD_TRIALS
        && invocations.len() == REQUIRED_COLD_TRIALS
        && submitted_trials == declared_slots
        && submitted_invocations == declared_slots;
    if !complete_sampling {
        reasons.push("required-sampling-incomplete".to_string());
    }
    let referenced_result_hashes = trials
        .iter()
        .flat_map(|trial| trial.evaluator_result_sha256s.iter())
        .collect::<HashSet<_>>();
    let unreferenced_results = result_by_hash
        .keys()
        .any(|hash| !referenced_result_hashes.contains(hash));
    if unreferenced_results {
        reasons.push("unreferenced-evaluator-result".to_string());
    }

    let unique_trial_ids = all_unique(trials.iter().map(|trial| trial.trial_id.as_str()))
        && all_unique(
            invocations
                .iter()
                .map(|invocation| invocation.trial_id.as_str()),
        );
    let unique_invocations = all_unique(
        invocations
            .iter()
            .map(|invocation| invocation.invocation_id.as_str()),
    );
    let unique_sessions = all_unique(
        invocations
            .iter()
            .map(|invocation| invocation.freshness.session_id.as_str()),
    );
    let unique_requests = all_unique(invocations.iter().filter_map(|invocation| {
        invocation
            .provider_metadata
            .as_ref()
            .and_then(|metadata| metadata.request_id.as_deref())
    }));
    let unique_outputs = all_unique(invocations.iter().filter_map(|invocation| {
        invocation
            .output
            .as_ref()
            .map(|output| output.sha256.as_str())
    }));
    if !(unique_trial_ids
        && unique_invocations
        && unique_sessions
        && unique_requests
        && unique_outputs)
    {
        q_reasons[0].push("trial-replay-or-identity-reuse".to_string());
    }

    for invocation in invocations {
        let receipt = invocation
            .freshness
            .verifier_receipt_sha256
            .as_ref()
            .and_then(|hash| verifier_by_hash.get(hash))
            .filter(|receipt| {
                inventory.trusted_verifiers.iter().any(|trusted| {
                    trusted.verifier_name == receipt.verifier_name
                        && trusted.verifier_version == receipt.verifier_version
                        && trusted.verifier_config_sha256 == receipt.verifier_config_sha256
                        && trusted.identity_authority_sha256 == receipt.identity_authority_sha256
                        && receipt.verify_signature(&trusted.public_key_hex).is_ok()
                }) && receipt.invocation_id == invocation.invocation_id
                    && receipt.candidate_sha256 == invocation.candidate_sha256
                    && receipt.evaluator_inventory_sha256 == invocation.evaluator_inventory_sha256
                    && receipt.model_visible_context_sha256
                        == invocation.model_visible_context_sha256
                    && receipt.started_at == invocation.started_at
                    && receipt.completed_at == invocation.completed_at
            });
        let slot = challenge
            .trial_slots
            .iter()
            .find(|slot| slot.trial_id == invocation.trial_id);
        if !slot.is_some_and(|slot| {
            slot.phase == invocation.phase
                && slot.requested_model == invocation.requested_model
                && slot.resolved_model == invocation.resolved_model
                && slot.prompt_sha256 == invocation.prompt_sha256
                && slot.input_artifacts == invocation.input_artifacts
                && slot.model_visible_context_sha256 == invocation.model_visible_context_sha256
        }) {
            q_reasons[1].push("model-visible-context-oracle-leak-or-hash-mismatch".to_string());
        }
        if invocation.job_id != candidate.job_id
            || invocation.fixture_id != candidate.fixture_id
            || invocation.candidate_sha256 != candidate_digest
            || invocation.evaluator_inventory_sha256 != inventory_digest
            || invocation.freshness.resumed
            || invocation.freshness.provenance != EvidenceProvenance::VerifierRecomputed
            || receipt.is_none()
        {
            q_reasons[0].push("fresh-host-binding-not-verified".to_string());
        }
        for required in COLD_ISOLATION_DIMENSIONS {
            let proven = receipt.is_some_and(|receipt| {
                receipt
                    .isolation_dimensions
                    .iter()
                    .any(|dimension| dimension == required)
            }) && invocation.isolation.iter().any(|observation| {
                observation.dimension == required
                    && matches!(
                        observation.state,
                        AssuranceEvidenceState::Enforced | AssuranceEvidenceState::Verified
                    )
                    && !matches!(
                        observation.provenance,
                        EvidenceProvenance::CustomerAttested
                            | EvidenceProvenance::HostAttested
                            | EvidenceProvenance::Unknown
                    )
                    && observation.verifier_receipt_sha256.as_ref()
                        == invocation.freshness.verifier_receipt_sha256.as_ref()
            });
            if !proven {
                q_reasons[0].push("cold-isolation-unproven".to_string());
            }
        }

        let context_digest = canonical_json_sha256_for_domain(
            "mdp.model-visible-context.v1",
            &serde_json::to_value(&invocation.input_artifacts)?,
        )?;
        let leaks_oracle = invocation.input_artifacts.iter().any(|input| {
            let name = input.name.to_ascii_lowercase();
            ["evaluator", "challenge", "expected", "score", "rubric"]
                .iter()
                .any(|forbidden| name.contains(forbidden))
        });
        if context_digest != invocation.model_visible_context_sha256 || leaks_oracle {
            q_reasons[1].push("model-visible-context-oracle-leak-or-hash-mismatch".to_string());
        }
        if invocation.started_at <= inventory.frozen_at
            || invocation.started_at <= challenge.created_at
        {
            q_reasons[2].push("challenge-not-frozen-before-trial".to_string());
        }
        if invocation
            .output
            .as_ref()
            .is_some_and(|output| output.lifecycle_policy_sha256 != lifecycle_digest)
        {
            q_reasons[3].push("output-lifecycle-policy-mismatch".to_string());
        }
    }

    if challenge.frozen_candidate_sha256 != candidate_freeze_sha256(candidate)?
        || challenge.prior_exposure != PriorExposure::NeverExposed
        || challenge.pack_authored
        || !challenge.protected
        || !challenge.frozen_before_trials
        || challenge.model_visible
    {
        q_reasons[2].push("protected-challenge-provenance-invalid".to_string());
    }
    if lifecycle.access_class == AccessClass::SanitizedPublic {
        for output in invocations.iter().filter_map(|item| item.output.as_ref()) {
            if !approvals.iter().any(|approval| {
                approval.approves_exact_hash(&output.sha256)
                    && inventory
                        .trusted_publication_authorities
                        .iter()
                        .any(|trusted| {
                            trusted.reviewer_role == approval.reviewer_role
                                && trusted.identity_authority_sha256
                                    == approval.identity_authority_sha256
                                && approval.verify_signature(&trusted.public_key_hex).is_ok()
                        })
            }) {
                q_reasons[3].push("sanitized-public-exact-hash-approval-missing".to_string());
            }
        }
    }

    let q_ids = ["Q1", "Q2", "Q3", "Q4"];
    let preflight_assertions = q_ids
        .into_iter()
        .zip(q_reasons.iter())
        .map(|(id, assertion_reasons)| ConformanceAssertionEvaluation {
            id: id.to_string(),
            status: if !assertion_reasons.is_empty() {
                AssertionEvaluationStatus::Failed
            } else if complete_sampling {
                AssertionEvaluationStatus::Passed
            } else {
                AssertionEvaluationStatus::NotApplicable
            },
            passed_trials: if assertion_reasons.is_empty() {
                trials.len().min(REQUIRED_COLD_TRIALS) as u8
            } else {
                0
            },
            required_trials: REQUIRED_COLD_TRIALS as u8,
            reason_codes: assertion_reasons.clone(),
        })
        .collect::<Vec<_>>();

    let mut binding_failures: std::collections::HashMap<&str, Vec<String>> =
        std::collections::HashMap::new();
    for trial in trials {
        let trial_reasons = binding_failures.entry(trial.trial_id.as_str()).or_default();
        let Some(invocation) = invocation_by_trial.get(trial.trial_id.as_str()).copied() else {
            trial_reasons.push("invocation-missing".to_string());
            continue;
        };
        let invocation_digest = canonical_authority_sha256(invocation)?;
        if trial.candidate_sha256 != candidate_digest
            || trial.invocation_sha256 != invocation_digest
            || trial.lifecycle_policy_sha256 != lifecycle_digest
            || trial.terminal_state != invocation.terminal_state
        {
            trial_reasons.push("trial-authority-binding-mismatch".to_string());
        }
        if lifecycle.access_class == AccessClass::SanitizedPublic {
            let exact_output_approved = invocation.output.as_ref().is_some_and(|output| {
                trial
                    .publication_approval_sha256s
                    .iter()
                    .any(|approval_hash| {
                        approval_by_hash.get(approval_hash).is_some_and(|approval| {
                            approval.approves_exact_hash(&output.sha256)
                                && inventory
                                    .trusted_publication_authorities
                                    .iter()
                                    .any(|trusted| {
                                        trusted.reviewer_role == approval.reviewer_role
                                            && trusted.identity_authority_sha256
                                                == approval.identity_authority_sha256
                                            && approval
                                                .verify_signature(&trusted.public_key_hex)
                                                .is_ok()
                                    })
                        })
                    })
            });
            if invocation.output.is_some() && !exact_output_approved {
                trial_reasons.push("publication-approval-binding-mismatch".to_string());
            }
        } else if !trial.publication_approval_sha256s.is_empty() {
            trial_reasons.push("unexpected-publication-approval".to_string());
        }
        if invocation.terminal_state != challenge.expected_terminal_state {
            trial_reasons.push("unexpected-terminal-state".to_string());
        }
        if trial.expected_bounded_non_success != !challenge.expected_terminal_state.is_success() {
            trial_reasons.push("bounded-non-success-expectation-mismatch".to_string());
        }
        let output_hash = invocation
            .output
            .as_ref()
            .map(|output| output.sha256.as_str());
        for result_hash in &trial.evaluator_result_sha256s {
            let Some(result) = result_by_hash.get(result_hash) else {
                trial_reasons.push("evaluator-result-missing-or-hash-mismatch".to_string());
                continue;
            };
            if result.trial_id != trial.trial_id
                || Some(result.output_sha256.as_str()) != output_hash
                || result.evaluator_inventory_sha256 != inventory_digest
                || result.evaluator_id != inventory.evaluator_id
                || result.evaluator_version != inventory.evaluator_version
            {
                trial_reasons.push("evaluator-result-binding-mismatch".to_string());
            }
        }
    }

    let mut behavioral_assertions = Vec::new();
    for assertion in &inventory.assertions {
        let mut passes = 0u8;
        let mut assertion_reasons = Vec::new();
        for trial in trials {
            if !binding_failures
                .get(trial.trial_id.as_str())
                .is_none_or(Vec::is_empty)
            {
                continue;
            }
            if trial.expected_bounded_non_success {
                passes += 1;
                continue;
            }
            let matching_scores = trial
                .evaluator_result_sha256s
                .iter()
                .filter_map(|hash| result_by_hash.get(hash))
                .flat_map(|result| {
                    result.scores.iter().filter_map(move |score| {
                        (score.assertion_id == assertion.assertion_id).then_some((result, score))
                    })
                })
                .collect::<Vec<_>>();
            if matching_scores.len() != 1 {
                assertion_reasons.push("missing-or-ambiguous-score".to_string());
                continue;
            }
            let (result, score) = matching_scores[0];
            let final_status = if score.status == ScoreStatus::Disputed {
                result
                    .adjudication
                    .as_ref()
                    .map(|adjudication| adjudication.decision)
                    .unwrap_or(ScoreStatus::Disputed)
            } else {
                score.status
            };
            if final_status == ScoreStatus::Pass {
                passes += 1;
            }
        }
        let status = if !complete_sampling || assertion_reasons.len() > 0 {
            AssertionEvaluationStatus::NotApplicable
        } else if passes >= assertion.minimum_passes {
            AssertionEvaluationStatus::Passed
        } else {
            assertion_reasons.push("sampling-threshold-not-met".to_string());
            AssertionEvaluationStatus::Failed
        };
        behavioral_assertions.push(ConformanceAssertionEvaluation {
            id: assertion.assertion_id.clone(),
            status,
            passed_trials: passes,
            required_trials: assertion.required_trials,
            reason_codes: assertion_reasons,
        });
    }

    let q_failed = preflight_assertions
        .iter()
        .any(|assertion| assertion.status == AssertionEvaluationStatus::Failed);
    let hard_failed = inventory.assertions.iter().any(|inventory_assertion| {
        inventory_assertion.kind == AssertionKind::HardBoundary
            && behavioral_assertions.iter().any(|evaluated| {
                evaluated.id == inventory_assertion.assertion_id
                    && evaluated.status == AssertionEvaluationStatus::Failed
            })
    });
    let any_behavior_failed = behavioral_assertions
        .iter()
        .any(|assertion| assertion.status == AssertionEvaluationStatus::Failed);
    let bindings_failed =
        unreferenced_results || binding_failures.values().any(|reasons| !reasons.is_empty());
    let behavioral_qualification = if deterministic_status != DeterministicStatus::Passed {
        BehavioralQualification::Unassessed
    } else if q_failed || hard_failed || any_behavior_failed || bindings_failed {
        BehavioralQualification::NotQualifiedForJobUnderEnvelope
    } else if !complete_sampling
        || behavioral_assertions
            .iter()
            .any(|assertion| assertion.status == AssertionEvaluationStatus::NotApplicable)
    {
        BehavioralQualification::Unassessed
    } else {
        BehavioralQualification::QualifiedForJobUnderEnvelope
    };

    let job_sufficiency = match deterministic_status {
        DeterministicStatus::Passed => JobSufficiency::SufficientForJob,
        DeterministicStatus::Failed => JobSufficiency::NotSufficientForJob,
        DeterministicStatus::Unassessed => JobSufficiency::Unassessed,
    };
    let overall_result = match (job_sufficiency, behavioral_qualification) {
        (JobSufficiency::NotSufficientForJob, _) => QualificationVerdict::NotSufficientForJob,
        (
            JobSufficiency::SufficientForJob,
            BehavioralQualification::QualifiedForJobUnderEnvelope,
        ) => QualificationVerdict::QualifiedForJobUnderEnvelope,
        (
            JobSufficiency::SufficientForJob,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope,
        ) => QualificationVerdict::NotQualifiedForJobUnderEnvelope,
        _ => QualificationVerdict::Unassessed,
    };
    let trial_sha256s = trials
        .iter()
        .map(canonical_authority_sha256)
        .collect::<Result<Vec<_>>>()?;
    let trials = trials
        .iter()
        .map(|trial| {
            let trial_reasons = binding_failures
                .remove(trial.trial_id.as_str())
                .unwrap_or_default();
            let status = if !trial_reasons.is_empty() {
                BehavioralStatus::Malformed
            } else if q_failed {
                BehavioralStatus::Failed
            } else if trial.expected_bounded_non_success {
                BehavioralStatus::BoundedNonSuccessConfirmed
            } else if trial.evaluator_result_sha256s.iter().any(|hash| {
                result_by_hash.get(hash).is_some_and(|result| {
                    result.scores.iter().any(|score| {
                        let final_status = if score.status == ScoreStatus::Disputed {
                            result
                                .adjudication
                                .as_ref()
                                .map(|adjudication| adjudication.decision)
                                .unwrap_or(ScoreStatus::Disputed)
                        } else {
                            score.status
                        };
                        final_status != ScoreStatus::Pass
                    })
                })
            }) {
                BehavioralStatus::Failed
            } else {
                BehavioralStatus::Passed
            };
            BehavioralTrialEvaluation {
                trial_id: trial.trial_id.clone(),
                status,
                usable_output: false,
                reason_codes: trial_reasons,
            }
        })
        .collect();
    reasons.extend(q_reasons.into_iter().flatten());
    reasons.extend(
        behavioral_assertions
            .iter()
            .flat_map(|assertion| assertion.reason_codes.iter().cloned()),
    );
    reasons.sort();
    reasons.dedup();

    let evaluation = BehavioralEvaluation {
        contract: BEHAVIORAL_EVALUATION_V1.to_string(),
        valid: matches!(
            behavioral_qualification,
            BehavioralQualification::QualifiedForJobUnderEnvelope
        ),
        job_id: candidate.job_id.clone(),
        candidate_sha256: candidate_digest,
        evaluator_inventory_sha256: inventory_digest.to_string(),
        lifecycle_policy_sha256: lifecycle_digest,
        deterministic_evaluation_sha256,
        trial_sha256s,
        deterministic_status,
        job_sufficiency,
        preflight_assertions,
        behavioral_assertions,
        trials,
        behavioral_qualification,
        overall_result,
        drafting_authority_granted: false,
        reason_codes: reasons,
    };
    evaluation.validate()?;
    Ok(evaluation)
}

fn all_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::{Value, json};

    fn hash(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(
            &decode_hex::<32>(
                "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
                "test key",
            )
            .unwrap(),
        )
    }

    fn hex_bytes(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn test_public_key_hex() -> String {
        hex_bytes(test_signing_key().verifying_key().as_bytes())
    }

    fn test_identity_sha256() -> String {
        crate::artifact_hash::sha256_hex(test_signing_key().verifying_key().as_bytes())
    }

    fn sign_test_authority<T: Clone + Serialize>(
        authority: &T,
        domain: &str,
        clear: impl FnOnce(&mut T),
    ) -> String {
        let mut unsigned = authority.clone();
        clear(&mut unsigned);
        let digest =
            canonical_json_sha256_for_domain(domain, &serde_json::to_value(unsigned).unwrap())
                .unwrap();
        let bytes = decode_hex::<32>(&digest, "digest").unwrap();
        hex_bytes(&test_signing_key().sign(&bytes).to_bytes())
    }

    #[test]
    fn candidate_is_closed_and_cannot_supply_an_expected_result() {
        let mut value = valid_candidate_value();
        value["expected_result"] = json!("pass");
        assert!(parse_candidate(&serde_json::to_vec(&value).unwrap()).is_err());

        value.as_object_mut().unwrap().remove("expected_result");
        assert!(parse_candidate(&serde_json::to_vec(&value).unwrap()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_relative_reader_rejects_symlinked_root_and_ancestor() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("mdp-openat-{}-{nonce}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("authority.json"), b"{}").unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        assert!(read_contained_file(&root, Path::new("linked/authority.json")).is_err());
        symlink(&root, base.join("root-link")).unwrap();
        assert!(read_contained_file(&base.join("root-link"), Path::new("authority.json")).is_err());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn invocation_retains_requested_and_resolved_models_and_rejects_forged_assurance() {
        let mut value = valid_invocation_value();
        let parsed = parse_invocation(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(parsed.requested_model, "requested-model");
        assert_eq!(parsed.resolved_model, "resolved-model");

        value["isolation"][0]["state"] = json!("verified");
        value["isolation"][0]["provenance"] = json!("host-attested");
        value["isolation"][0]["verifier_receipt_sha256"] = Value::Null;
        assert!(parse_invocation(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn evaluator_inventory_hashes_canonically_and_keeps_challenges_out_of_context() {
        let mut value = valid_inventory_value();
        let digest = hash_authority_value(EVALUATOR_INVENTORY_V1, &value).unwrap();
        value["inventory_sha256"] = json!(digest);
        assert!(parse_evaluator_inventory(&serde_json::to_vec(&value).unwrap()).is_ok());

        value["challenges"][0]["model_visible"] = json!(true);
        let digest = hash_authority_value(EVALUATOR_INVENTORY_V1, &value).unwrap();
        value["inventory_sha256"] = json!(digest);
        assert!(parse_evaluator_inventory(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn private_lifecycle_and_publication_approval_fail_closed() {
        let mut policy = valid_lifecycle_value();
        policy["host_capabilities"]["deletion"] = json!("unsupported");
        assert!(parse_lifecycle_policy(&serde_json::to_vec(&policy).unwrap()).is_err());

        let approval = valid_publication_approval_value();
        let parsed = parse_publication_approval(&serde_json::to_vec(&approval).unwrap()).unwrap();
        assert!(parsed.approves_exact_hash(&hash('a')));
        assert!(!parsed.approves_exact_hash(&hash('b')));
        assert!(parsed.verify_signature(&test_public_key_hex()).is_ok());
        let mut forged = parsed.clone();
        forged.signature_hex = "00".repeat(64);
        assert!(forged.verify_signature(&test_public_key_hex()).is_err());
        let mut altered = parsed;
        altered.artifact_sha256 = hash('b');
        assert!(altered.verify_signature(&test_public_key_hex()).is_err());
    }

    #[test]
    fn verifier_receipt_signature_rejects_forgery_and_post_sign_tampering() {
        let evidence = behavioral_evidence(TerminalState::Success, 3, 1);
        let receipt = &evidence.verifier_receipts[0];
        assert!(receipt.verify_signature(&test_public_key_hex()).is_ok());
        let mut forged = receipt.clone();
        forged.signature_hex = "00".repeat(64);
        assert!(forged.verify_signature(&test_public_key_hex()).is_err());
        let mut altered = receipt.clone();
        altered.invocation_id = "different-invocation".into();
        assert!(altered.verify_signature(&test_public_key_hex()).is_err());
    }

    #[test]
    fn evaluator_dispute_requires_independent_named_human_adjudication() {
        let mut value = valid_evaluator_result_value();
        value["disagreement"] = json!("open");
        value["adjudication"] = Value::Null;
        assert!(parse_evaluator_result(&serde_json::to_vec(&value).unwrap()).is_err());

        value["disagreement"] = json!("resolved");
        value["competing_score_sha256s"] = json!([hash('1'), hash('2')]);
        value["adjudication"] = json!({
            "adjudicator_name": "Independent Reviewer",
            "reviewer_role": "independent-customer-adjudicator",
            "identity_authority_ref": "review-authority:42",
            "approval_receipt_sha256": hash('3'),
            "output_sha256": hash('f'),
            "competing_score_sha256s": [hash('1'), hash('2')],
            "decision": "pass",
            "purpose": "resolve-hard-boundary",
            "approved_at": "2026-08-13T12:00:00Z"
        });
        assert!(parse_evaluator_result(&serde_json::to_vec(&value).unwrap()).is_ok());
    }

    #[test]
    fn parsers_enforce_resource_limits_before_deserialization() {
        let oversized = vec![b' '; MAX_CONFORMANCE_AUTHORITY_BYTES + 1];
        assert!(parse_candidate(&oversized).is_err());
        let mut value = valid_invocation_value();
        value["input_artifacts"] = Value::Array(
            (0..=MAX_MODEL_VISIBLE_INPUTS)
                .map(|index| json!({"name": format!("input-{index}"), "sha256": hash('a')}))
                .collect(),
        );
        assert!(parse_invocation(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn authority_inputs_reject_partial_inventory_and_offset_timestamps() {
        let mut inventory = valid_inventory_value();
        inventory["assertions"].as_array_mut().unwrap().pop();
        let digest = hash_authority_value(EVALUATOR_INVENTORY_V1, &inventory).unwrap();
        inventory["inventory_sha256"] = json!(digest);
        assert!(parse_evaluator_inventory(&serde_json::to_vec(&inventory).unwrap()).is_err());

        let mut invocation = valid_invocation_value();
        invocation["started_at"] = json!("2026-08-13T08:00:00-04:00");
        assert!(parse_invocation(&serde_json::to_vec(&invocation).unwrap()).is_err());

        for invalid in [
            "2026-08-13T12:00Z",
            "2026-08-13T12:00:00.000Z",
            "2026-8-13T12:00:00Z",
        ] {
            let mut invocation = valid_invocation_value();
            invocation["started_at"] = json!(invalid);
            assert!(parse_invocation(&serde_json::to_vec(&invocation).unwrap()).is_err());
        }
        assert!(parse_invocation(&serde_json::to_vec(&valid_invocation_value()).unwrap()).is_ok());
    }

    #[test]
    fn wrong_frozen_slot_and_nonexistent_verifier_receipt_fail_qualification() {
        let mut wrong_slot = behavioral_evidence(TerminalState::Success, 3, 3);
        wrong_slot.invocations[0].prompt_sha256 = hash('9');
        wrong_slot.trials[0].invocation_sha256 =
            canonical_authority_sha256(&wrong_slot.invocations[0]).unwrap();
        let evaluation = wrong_slot.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );

        let mut wrong_model = behavioral_evidence(TerminalState::Success, 3, 3);
        wrong_model.invocations[0].resolved_model = "different-model".into();
        wrong_model.trials[0].invocation_sha256 =
            canonical_authority_sha256(&wrong_model.invocations[0]).unwrap();
        assert_eq!(
            wrong_model.evaluate(3).unwrap().behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );

        let mut untrusted_verifier = behavioral_evidence(TerminalState::Success, 3, 3);
        untrusted_verifier.verifier_receipts[0].identity_authority_sha256 = hash('0');
        let receipt_hash =
            canonical_authority_sha256(&untrusted_verifier.verifier_receipts[0]).unwrap();
        untrusted_verifier.invocations[0]
            .freshness
            .verifier_receipt_sha256 = Some(receipt_hash.clone());
        for observation in &mut untrusted_verifier.invocations[0].isolation {
            observation.verifier_receipt_sha256 = Some(receipt_hash.clone());
        }
        untrusted_verifier.trials[0].invocation_sha256 =
            canonical_authority_sha256(&untrusted_verifier.invocations[0]).unwrap();
        assert_eq!(
            untrusted_verifier
                .evaluate(3)
                .unwrap()
                .behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );

        let mut missing_receipt = behavioral_evidence(TerminalState::Success, 3, 3);
        missing_receipt.invocations[0]
            .freshness
            .verifier_receipt_sha256 = Some(hash('0'));
        for observation in &mut missing_receipt.invocations[0].isolation {
            observation.verifier_receipt_sha256 = Some(hash('0'));
        }
        missing_receipt.trials[0].invocation_sha256 =
            canonical_authority_sha256(&missing_receipt.invocations[0]).unwrap();
        let evaluation = missing_receipt.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );
    }

    #[test]
    fn behavioral_thresholds_require_hard_three_of_three_and_useful_two_of_three() {
        let evidence = behavioral_evidence(TerminalState::Success, 2, 3);
        let evaluation = evidence.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::QualifiedForJobUnderEnvelope
        );
        assert!(!evaluation.drafting_authority_granted);
        let bytes = serde_json::to_vec(&evaluation).unwrap();
        assert!(parse_behavioral_evaluation(&bytes).is_ok());
        let mut unknown = serde_json::to_value(&evaluation).unwrap();
        unknown["provider_response"] = json!("must remain private");
        assert!(parse_behavioral_evaluation(&serde_json::to_vec(&unknown).unwrap()).is_err());

        let weak = behavioral_evidence(TerminalState::Success, 1, 3);
        let evaluation = weak.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );
        assert_eq!(
            evaluation
                .behavioral_assertions
                .iter()
                .find(|assertion| assertion.id == "B6")
                .unwrap()
                .status,
            AssertionEvaluationStatus::Failed
        );

        let mut hard_failure = behavioral_evidence(TerminalState::Success, 3, 3);
        hard_failure.results[1]
            .scores
            .iter_mut()
            .find(|score| score.assertion_id == "B1")
            .unwrap()
            .status = ScoreStatus::Fail;
        hard_failure.trials[1].evaluator_result_sha256s =
            vec![canonical_authority_sha256(&hard_failure.results[1]).unwrap()];
        let evaluation = hard_failure.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );
    }

    #[test]
    fn hand_authored_empty_qualified_behavioral_aggregate_is_rejected() {
        let evidence = behavioral_evidence(TerminalState::Success, 3, 3);
        let mut evaluation = evidence.evaluate(3).unwrap();
        assert!(evaluation.valid);
        evaluation.trials.clear();
        evaluation.trial_sha256s.clear();
        assert!(evaluation.validate().is_err());

        let mut wrong_threshold = evidence.evaluate(3).unwrap();
        wrong_threshold.behavioral_assertions[5].passed_trials = 1;
        assert!(wrong_threshold.validate().is_err());

        let mut failed_trial = evidence.evaluate(3).unwrap();
        failed_trial.trials[0].status = BehavioralStatus::Failed;
        assert!(failed_trial.validate().is_err());
    }

    #[test]
    fn missing_predeclared_trial_slot_is_unassessed() {
        let evidence = behavioral_evidence(TerminalState::Success, 2, 3);
        let evaluation = evidence.evaluate(2).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::Unassessed
        );
        assert!(
            evaluation
                .reason_codes
                .contains(&"required-sampling-incomplete".to_string())
        );

        let complete = behavioral_evidence(TerminalState::Success, 3, 3);
        let evaluation = complete
            .evaluate_with_status(3, DeterministicStatus::Failed)
            .unwrap();
        assert_eq!(
            evaluation.job_sufficiency,
            JobSufficiency::NotSufficientForJob
        );
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::Unassessed
        );
        assert_eq!(
            evaluation.overall_result,
            QualificationVerdict::NotSufficientForJob
        );

        let mut incomplete_failure = behavioral_evidence(TerminalState::Success, 2, 3);
        incomplete_failure.invocations[0].isolation[0].state = AssuranceEvidenceState::Unknown;
        incomplete_failure.invocations[0].isolation[0].provenance = EvidenceProvenance::Unknown;
        incomplete_failure.invocations[0].isolation[0].verifier_receipt_sha256 = None;
        incomplete_failure.trials[0].invocation_sha256 =
            canonical_authority_sha256(&incomplete_failure.invocations[0]).unwrap();
        let evaluation = incomplete_failure.evaluate(2).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );
        assert!(evaluation.validate().is_ok());
    }

    #[test]
    fn replay_under_new_ids_and_unknown_isolation_fail_closed() {
        let mut replay = behavioral_evidence(TerminalState::Success, 3, 3);
        replay.invocations[1].freshness.session_id =
            replay.invocations[0].freshness.session_id.clone();
        replay.invocations[1].output.as_mut().unwrap().sha256 = replay.invocations[0]
            .output
            .as_ref()
            .unwrap()
            .sha256
            .clone();
        let evaluation = replay.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );
        assert!(
            evaluation
                .reason_codes
                .iter()
                .any(|reason| reason == "trial-replay-or-identity-reuse")
        );

        let mut unknown = behavioral_evidence(TerminalState::Success, 3, 3);
        unknown.invocations[0].isolation[0].state = AssuranceEvidenceState::Unknown;
        unknown.invocations[0].isolation[0].provenance = EvidenceProvenance::Unknown;
        unknown.invocations[0].isolation[0].verifier_receipt_sha256 = None;
        let evaluation = unknown.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope
        );
        assert!(
            evaluation
                .reason_codes
                .iter()
                .any(|reason| reason.starts_with("cold-isolation-unproven"))
        );

        let mut forged = valid_invocation_value();
        forged["isolation"][0]["state"] = json!("enforced");
        forged["isolation"][0]["provenance"] = json!("host-attested");
        assert!(parse_invocation(&serde_json::to_vec(&forged).unwrap()).is_err());

        let mut oracle = behavioral_evidence(TerminalState::Success, 3, 3);
        oracle.invocations[0].input_artifacts[0].name = "evaluator-rubric".to_string();
        oracle.invocations[0].model_visible_context_sha256 = canonical_json_sha256_for_domain(
            "mdp.model-visible-context.v1",
            &serde_json::to_value(&oracle.invocations[0].input_artifacts).unwrap(),
        )
        .unwrap();
        oracle.trials[0].invocation_sha256 =
            canonical_authority_sha256(&oracle.invocations[0]).unwrap();
        let evaluation = oracle.evaluate(3).unwrap();
        assert_eq!(
            evaluation.preflight_assertions[1].status,
            AssertionEvaluationStatus::Failed
        );
    }

    #[test]
    fn expected_bounded_non_success_counts_without_creating_usable_output() {
        let evidence = behavioral_evidence(TerminalState::NoDraftPreflightRefused, 0, 3);
        let evaluation = evidence.evaluate(3).unwrap();
        assert_eq!(
            evaluation.behavioral_qualification,
            BehavioralQualification::QualifiedForJobUnderEnvelope
        );
        assert!(evaluation.trials.iter().all(|trial| {
            trial.status == BehavioralStatus::BoundedNonSuccessConfirmed && !trial.usable_output
        }));
        assert!(!evaluation.drafting_authority_granted);
    }

    #[test]
    fn adjudication_rejects_wrong_role_and_accepts_independent_receipt() {
        let mut value = valid_evaluator_result_value();
        value["disagreement"] = json!("resolved");
        value["competing_score_sha256s"] = json!([hash('1'), hash('2')]);
        value["scores"][0]["status"] = json!("disputed");
        value["adjudication"] = json!({
            "adjudicator_name":"Release Author","reviewer_role":"release-approver",
            "identity_authority_ref":"review-authority:42","approval_receipt_sha256":hash('3'),"output_sha256":hash('f'),
            "competing_score_sha256s":[hash('1'),hash('2')],"decision":"pass",
            "purpose":"resolve-hard-boundary","approved_at":"2026-08-13T12:00:00Z"
        });
        assert!(parse_evaluator_result(&serde_json::to_vec(&value).unwrap()).is_err());
        value["adjudication"]["reviewer_role"] = json!("independent-customer-adjudicator");
        assert!(parse_evaluator_result(&serde_json::to_vec(&value).unwrap()).is_ok());
    }

    struct BehavioralEvidence {
        candidate: ConformanceCandidateV1,
        inventory: EvaluatorInventoryV1,
        lifecycle: PrivateRecordPolicyV1,
        invocations: Vec<ModelInvocationEvidenceV1>,
        trials: Vec<ConformanceTrialV1>,
        results: Vec<EvaluatorResultV1>,
        verifier_receipts: Vec<ConformanceVerifierReceiptV1>,
    }

    impl BehavioralEvidence {
        fn evaluate(&self, count: usize) -> Result<BehavioralEvaluation> {
            self.evaluate_with_status(count, DeterministicStatus::Passed)
        }

        fn evaluate_with_status(
            &self,
            count: usize,
            deterministic_status: DeterministicStatus,
        ) -> Result<BehavioralEvaluation> {
            let assertion_status = match deterministic_status {
                DeterministicStatus::Passed => "pass",
                DeterministicStatus::Failed => "fail",
                DeterministicStatus::Unassessed => "unassessed",
            };
            let verdict = match deterministic_status {
                DeterministicStatus::Passed => DeterministicVerdict::SufficientForJob,
                DeterministicStatus::Failed => DeterministicVerdict::NotSufficientForJob,
                DeterministicStatus::Unassessed => DeterministicVerdict::Unassessed,
            };
            let deterministic = DeterministicConformanceV1 {
                contract: DETERMINISTIC_CONFORMANCE_V1.into(),
                valid: deterministic_status == DeterministicStatus::Passed,
                candidate_id: self.candidate.candidate_id.clone(),
                job_id: self.candidate.job_id.clone(),
                pack_release: self.candidate.pack_release.clone(),
                evaluator: DeterministicEvaluatorIdentity {
                    id: self.inventory.evaluator_id.clone(),
                    version: self.inventory.evaluator_version.clone(),
                    fixture_set_id: self.inventory.fixture_set_id.clone(),
                    inventory_sha256: self.inventory.inventory_sha256.clone(),
                },
                fixture_id: self.candidate.fixture_id.clone(),
                challenge_id: self.candidate.challenge_id.clone(),
                status: verdict,
                behavioral_qualification_allowed: deterministic_status
                    == DeterministicStatus::Passed,
                assertions: (1..=12)
                    .map(|n| DeterministicAssertion {
                        id: format!("D{n}"),
                        name: format!("assertion-{n}"),
                        scope: "fixture".into(),
                        hard: true,
                        status: assertion_status.into(),
                        authority_refs: vec![],
                        reason_codes: vec!["test-result".into()],
                    })
                    .collect(),
                summary: DeterministicSummary {
                    passed: if deterministic_status == DeterministicStatus::Passed {
                        12
                    } else {
                        0
                    },
                    failed: if deterministic_status == DeterministicStatus::Failed {
                        12
                    } else {
                        0
                    },
                    unassessed: if deterministic_status == DeterministicStatus::Unassessed {
                        12
                    } else {
                        0
                    },
                },
            };
            evaluate_behavioral_trials(
                &self.candidate,
                &self.inventory,
                &self.lifecycle,
                &deterministic,
                &self.invocations[..count],
                &self.trials[..count],
                &self.results[..count.min(self.results.len())],
                &[],
                &self.verifier_receipts[..count],
            )
        }
    }

    fn behavioral_evidence(
        terminal_state: TerminalState,
        useful_passes: usize,
        count: usize,
    ) -> BehavioralEvidence {
        let lifecycle =
            parse_lifecycle_policy(&serde_json::to_vec(&valid_lifecycle_value()).unwrap()).unwrap();
        let lifecycle_digest = canonical_authority_sha256(&lifecycle).unwrap();
        let mut candidate_value = valid_candidate_value();
        let provisional_candidate =
            parse_candidate(&serde_json::to_vec(&candidate_value).unwrap()).unwrap();
        let freeze_digest = candidate_freeze_sha256(&provisional_candidate).unwrap();
        let mut inventory_value = valid_inventory_value();
        inventory_value["challenges"][0]["frozen_candidate_sha256"] = json!(freeze_digest);
        inventory_value["challenges"][0]["expected_terminal_state"] =
            serde_json::to_value(terminal_state).unwrap();
        inventory_value["assertions"] = Value::Array(
            (1..=9)
                .map(|number| {
                    json!({
                        "assertion_id": format!("B{number}"),
                        "kind": if number == 6 { "useful-completion" } else { "hard-boundary" },
                        "required_trials": 3,
                        "minimum_passes": if number == 6 { 2 } else { 3 }
                    })
                })
                .collect(),
        );
        let inventory_digest =
            hash_authority_value(EVALUATOR_INVENTORY_V1, &inventory_value).unwrap();
        inventory_value["inventory_sha256"] = json!(inventory_digest);
        let inventory =
            parse_evaluator_inventory(&serde_json::to_vec(&inventory_value).unwrap()).unwrap();
        candidate_value["evaluator_inventory_sha256"] = json!(inventory.inventory_sha256.clone());
        candidate_value["authorities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|authority| authority["role"] == "evaluator-inventory")
            .unwrap()["sha256"] = json!(inventory.inventory_sha256.clone());
        let candidate = parse_candidate(&serde_json::to_vec(&candidate_value).unwrap()).unwrap();
        assert_eq!(candidate_freeze_sha256(&candidate).unwrap(), freeze_digest);
        let candidate_digest = canonical_authority_sha256(&candidate).unwrap();

        let mut invocations = Vec::new();
        let mut trials = Vec::new();
        let mut results = Vec::new();
        let mut verifier_receipts = Vec::new();
        for index in 0..count {
            let trial_id = format!("trial-{}", index + 1);
            let output_digest = hash(char::from(b'a' + index as u8));
            let inputs = vec![ModelVisibleInput {
                name: "prospect".to_string(),
                sha256: hash('d'),
            }];
            let context_digest = canonical_json_sha256_for_domain(
                "mdp.model-visible-context.v1",
                &serde_json::to_value(&inputs).unwrap(),
            )
            .unwrap();
            let output = terminal_state.is_success().then(|| PrivateArtifactRef {
                artifact_id: format!("private-output-{}", index + 1),
                sha256: output_digest.clone(),
                byte_count: 128,
                lifecycle_policy_sha256: lifecycle_digest.clone(),
            });
            let invocation = ModelInvocationEvidenceV1 {
                contract: MODEL_INVOCATION_EVIDENCE_V1.to_string(),
                invocation_id: format!("invocation-{}", index + 1),
                trial_id: trial_id.clone(),
                phase: InvocationPhase::Generation,
                job_id: candidate.job_id.clone(),
                fixture_id: candidate.fixture_id.clone(),
                candidate_sha256: candidate_digest.clone(),
                evaluator_inventory_sha256: inventory.inventory_sha256.clone(),
                requested_model: "requested-model".to_string(),
                resolved_model: "resolved-model".to_string(),
                prompt_sha256: hash('c'),
                input_artifacts: inputs,
                model_visible_context_sha256: context_digest,
                started_at: format!("2026-08-13T12:0{index}:00Z"),
                completed_at: format!("2026-08-13T12:0{index}:30Z"),
                freshness: FreshnessEvidence {
                    session_id: format!("verified-session-{}", index + 1),
                    resumed: false,
                    provenance: EvidenceProvenance::VerifierRecomputed,
                    verifier_receipt_sha256: Some(hash(char::from(b'4' + index as u8))),
                },
                isolation: COLD_ISOLATION_DIMENSIONS
                    .iter()
                    .map(|dimension| IsolationObservation {
                        dimension: (*dimension).to_string(),
                        state: AssuranceEvidenceState::Verified,
                        provenance: EvidenceProvenance::VerifierRecomputed,
                        evidence_refs: vec![format!("verifier:{}:{dimension}", index + 1)],
                        limitations: vec![],
                        verifier_receipt_sha256: Some(hash(char::from(b'4' + index as u8))),
                    })
                    .collect(),
                provider_metadata: Some(ProviderMetadata {
                    request_id: Some(format!("request-{}", index + 1)),
                    region: None,
                }),
                terminal_state,
                output,
            };
            verifier_receipts.push(ConformanceVerifierReceiptV1 {
                contract: CONFORMANCE_VERIFIER_RECEIPT_V1.into(),
                receipt_id: format!("receipt-{}", index + 1),
                verifier_name: "local-verifier".into(),
                verifier_version: "1.0.0".into(),
                verifier_config_sha256: hash('7'),
                identity_authority_sha256: test_identity_sha256(),
                invocation_id: invocation.invocation_id.clone(),
                candidate_sha256: invocation.candidate_sha256.clone(),
                evaluator_inventory_sha256: invocation.evaluator_inventory_sha256.clone(),
                model_visible_context_sha256: invocation.model_visible_context_sha256.clone(),
                started_at: invocation.started_at.clone(),
                completed_at: invocation.completed_at.clone(),
                freshness_verified: true,
                isolation_dimensions: COLD_ISOLATION_DIMENSIONS
                    .iter()
                    .map(|v| (*v).into())
                    .collect(),
                signature_hex: String::new(),
            });
            verifier_receipts.last_mut().unwrap().signature_hex = sign_test_authority(
                verifier_receipts.last().unwrap(),
                "mdp.conformance-verifier-receipt.v1.signature.v1",
                |value| value.signature_hex.clear(),
            );
            let verifier_hash =
                canonical_authority_sha256(verifier_receipts.last().unwrap()).unwrap();
            // The invocation references the exact contained verifier receipt.
            let mut invocation = invocation;
            invocation.freshness.verifier_receipt_sha256 = Some(verifier_hash.clone());
            for observation in &mut invocation.isolation {
                observation.verifier_receipt_sha256 = Some(verifier_hash.clone());
            }
            let invocation_digest = canonical_authority_sha256(&invocation).unwrap();
            let mut evaluator_hashes = Vec::new();
            if terminal_state.is_success() {
                let result = EvaluatorResultV1 {
                    contract: EVALUATOR_RESULT_V1.to_string(),
                    result_id: format!("result-{}", index + 1),
                    trial_id: trial_id.clone(),
                    output_sha256: output_digest,
                    evaluator_inventory_sha256: inventory.inventory_sha256.clone(),
                    evaluator_id: inventory.evaluator_id.clone(),
                    evaluator_version: inventory.evaluator_version.clone(),
                    scorer: ScorerIdentity {
                        scorer_type: ScorerType::NamedHuman,
                        scorer_id: format!("reviewer-{}", index + 1),
                        reviewer_role: "customer-reviewer".to_string(),
                        identity_authority_ref: Some(format!("review-authority:{}", index + 1)),
                    },
                    scores: (1..=9)
                        .map(|number| AssertionScore {
                            assertion_id: format!("B{number}"),
                            status: if number != 6 || index < useful_passes {
                                ScoreStatus::Pass
                            } else {
                                ScoreStatus::Fail
                            },
                            rationale: "Externally inspected.".to_string(),
                        })
                        .collect(),
                    competing_score_sha256s: vec![],
                    disagreement: DisagreementState::None,
                    adjudication: None,
                };
                evaluator_hashes.push(canonical_authority_sha256(&result).unwrap());
                results.push(result);
            }
            trials.push(ConformanceTrialV1 {
                contract: CONFORMANCE_TRIAL_V1.to_string(),
                trial_id: trial_id.clone(),
                candidate_sha256: candidate_digest.clone(),
                invocation_sha256: invocation_digest,
                evaluator_result_sha256s: evaluator_hashes,
                terminal_state,
                useful_completion: terminal_state.is_success().then_some(index < useful_passes),
                expected_bounded_non_success: !terminal_state.is_success(),
                lifecycle_policy_sha256: lifecycle_digest.clone(),
                publication_approval_sha256s: vec![],
            });
            invocations.push(invocation);
        }
        BehavioralEvidence {
            candidate,
            inventory,
            lifecycle,
            invocations,
            trials,
            results,
            verifier_receipts,
        }
    }

    fn valid_candidate_value() -> Value {
        json!({
            "contract": CONFORMANCE_CANDIDATE_V1,
            "candidate_id": "candidate-1",
            "artifact_root": "candidate",
            "job_id": "outbound-copy-brief",
            "pack_release": {"pack_id":"pack","release_id":"release","version":"1.0.0","portable_digest":hash('a'),"source_revision":hash('b')},
            "cli_version": "0.1.0",
            "fixture_id": "fixture-1",
            "challenge_id": "challenge-1",
            "evaluator_inventory_sha256": hash('c'),
            "authorities": [
                {"role":"pack-manifest","contract":"mdp.v0","relative_path":"pack/manifest.json","sha256":hash('d'),"byte_count":100},
                {"role":"requirements","contract":"mdp.requirements.v2","relative_path":"pack/requirements.json","sha256":hash('e'),"byte_count":100},
                {"role":"prompt","contract":"mdp.prompt.v1","relative_path":"pack/prompt.json","sha256":hash('f'),"byte_count":100},
                {"role":"evaluator-inventory","contract":EVALUATOR_INVENTORY_V1,"relative_path":"evaluator/inventory.json","sha256":hash('c'),"byte_count":100},
                {"role":"private-record-policy","contract":PRIVATE_RECORD_POLICY_V1,"relative_path":"policy/private.json","sha256":hash('9'),"byte_count":100}
            ],
            "lifecycle_policy_sha256": hash('9')
        })
    }

    fn valid_invocation_value() -> Value {
        json!({
            "contract": MODEL_INVOCATION_EVIDENCE_V1,
            "invocation_id":"invocation-1","trial_id":"trial-1","phase":"generation",
            "job_id":"outbound-copy-brief","fixture_id":"fixture-1","candidate_sha256":hash('a'),
            "evaluator_inventory_sha256":hash('b'),"requested_model":"requested-model","resolved_model":"resolved-model",
            "prompt_sha256":hash('c'),"input_artifacts":[{"name":"prospect","sha256":hash('d')}],
            "model_visible_context_sha256":hash('e'),"started_at":"2026-08-13T12:00:00Z","completed_at":"2026-08-13T12:01:00Z",
            "freshness":{"session_id":"opaque-session","resumed":false,"provenance":"verifier-recomputed","verifier_receipt_sha256":hash('f')},
            "isolation":[{"dimension":"memory","state":"verified","provenance":"verifier-recomputed","evidence_refs":["verifier:1"],"limitations":[],"verifier_receipt_sha256":hash('f')}],
            "provider_metadata":{"request_id":"opaque-request","region":null},
            "terminal_state":"success","output":{"artifact_id":"private-output-1","sha256":hash('f'),"byte_count":128,"lifecycle_policy_sha256":hash('9')}
        })
    }

    fn valid_inventory_value() -> Value {
        let slot_inputs = json!([{"name":"prospect","sha256":hash('d')}]);
        let slot_context =
            canonical_json_sha256_for_domain("mdp.model-visible-context.v1", &slot_inputs).unwrap();
        let mut value = json!({
            "contract":EVALUATOR_INVENTORY_V1,"evaluator_id":"cold-model","evaluator_version":"1.0.0","fixture_set_id":"core",
            "frozen_at":"2026-08-13T10:00:00Z","inventory_sha256":"",
            "trusted_verifiers":[{"verifier_name":"local-verifier","verifier_version":"1.0.0","verifier_config_sha256":hash('7'),"identity_authority_sha256":test_identity_sha256(),"public_key_hex":test_public_key_hex()}],
            "trusted_publication_authorities":[{"reviewer_role":"publication-reviewer","identity_authority_sha256":test_identity_sha256(),"public_key_hex":test_public_key_hex()}],
            "challenges":[{"challenge_id":"challenge-1","fixture_id":"fixture-1","job_id":"outbound-copy-brief","phase":"generation","expected_terminal_state":"success","protected":true,"frozen_before_trials":true,"model_visible":false,"selection_method":"seeded-synthetic","selection_version":"1.0.0","created_at":"2026-08-13T09:00:00Z","frozen_candidate_sha256":hash('4'),"selection_receipt_sha256":hash('5'),"prior_exposure":"never-exposed","pack_authored":false,"reuse_allowed":true,
                "trial_slots":[
                    {"trial_id":"trial-1","phase":"generation","requested_model":"requested-model","resolved_model":"resolved-model","prompt_sha256":hash('c'),"input_artifacts":slot_inputs,"model_visible_context_sha256":slot_context},
                    {"trial_id":"trial-2","phase":"generation","requested_model":"requested-model","resolved_model":"resolved-model","prompt_sha256":hash('c'),"input_artifacts":[{"name":"prospect","sha256":hash('d')}],"model_visible_context_sha256":slot_context},
                    {"trial_id":"trial-3","phase":"generation","requested_model":"requested-model","resolved_model":"resolved-model","prompt_sha256":hash('c'),"input_artifacts":[{"name":"prospect","sha256":hash('d')}],"model_visible_context_sha256":slot_context}
                ]}],
            "assertions":[]
        });
        value["assertions"] = Value::Array(
            (1..=9)
                .map(|number| {
                    json!({
                        "assertion_id":format!("B{number}"),
                        "kind":if number == 6 { "useful-completion" } else { "hard-boundary" },
                        "required_trials":3,
                        "minimum_passes":if number == 6 { 2 } else { 3 }
                    })
                })
                .collect(),
        );
        value
    }

    fn valid_lifecycle_value() -> Value {
        json!({"contract":PRIVATE_RECORD_POLICY_V1,"policy_id":"policy-1","access_class":"private","policy_owner_or_ref":"owner:security","retention_until":"2026-09-13T00:00:00Z","deletion_disposition":"delete","host_capabilities":{"access":"supported","retention":"supported","deletion":"supported"}})
    }

    fn valid_publication_approval_value() -> Value {
        let mut approval = PublicationApprovalV1 {
            contract: PUBLICATION_APPROVAL_V1.into(),
            approval_id: "approval-1".into(),
            artifact_sha256: hash('a'),
            classification: AccessClass::SanitizedPublic,
            approved_by: "Named Reviewer".into(),
            reviewer_role: "publication-reviewer".into(),
            identity_authority_sha256: test_identity_sha256(),
            approved_at: "2026-08-13T12:00:00Z".into(),
            purpose: "public-conformance-report".into(),
            signature_hex: String::new(),
        };
        approval.signature_hex = sign_test_authority(
            &approval,
            "mdp.publication-approval.v1.signature.v1",
            |value| value.signature_hex.clear(),
        );
        serde_json::to_value(approval).unwrap()
    }

    fn valid_evaluator_result_value() -> Value {
        json!({"contract":EVALUATOR_RESULT_V1,"result_id":"result-1","trial_id":"trial-1","output_sha256":hash('f'),"evaluator_inventory_sha256":hash('e'),"evaluator_id":"cold-model","evaluator_version":"1.0.0","scorer":{"scorer_type":"named-human","scorer_id":"reviewer-1","reviewer_role":"customer-reviewer","identity_authority_ref":"review-authority:1"},"scores":[{"assertion_id":"B1","status":"pass","rationale":"The synthetic output follows the boundary."}],"competing_score_sha256s":[],"disagreement":"none","adjudication":null})
    }
}
