use crate::decision_input::AdapterKind;

pub(crate) const PACKAGED_SKILL_IDS: [&str; 4] = [
    "mdp",
    "mdp-pack-builder",
    "mdp-pack-review",
    "mdp-pack-apply",
];
pub(crate) const BOOTSTRAP_SKILL_IDS: [&str; 3] = ["mdp", "mdp-pack-builder", "mdp-pack-review"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JobRouteSpec {
    pub(crate) job_id: &'static str,
    pub(crate) skill_id: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProfileDescriptor<'a> {
    pub(crate) profile_id: &'static str,
    pub(crate) jobs: &'a [JobRouteSpec],
    pub(crate) input_adapter: Option<AdapterKind>,
    pub(crate) template_id: &'static str,
}

const GTM_JOBS: &[JobRouteSpec] = &[
    JobRouteSpec {
        job_id: "prospect-fit-or-brief",
        skill_id: "mdp-pack-apply",
    },
    JobRouteSpec {
        job_id: "outbound-copy-brief",
        skill_id: "mdp-pack-apply",
    },
    JobRouteSpec {
        job_id: "outbound-copy-review",
        skill_id: "mdp-pack-apply",
    },
];
const PROPOSAL_JOBS: &[JobRouteSpec] = &[
    JobRouteSpec {
        job_id: "bid-no-bid-review",
        skill_id: "mdp-pack-apply",
    },
    JobRouteSpec {
        job_id: "compliance-review",
        skill_id: "mdp-pack-apply",
    },
    JobRouteSpec {
        job_id: "proof-review",
        skill_id: "mdp-pack-apply",
    },
    JobRouteSpec {
        job_id: "red-team-review",
        skill_id: "mdp-pack-apply",
    },
];

pub(crate) const PROFILE_DESCRIPTORS: &[ProfileDescriptor<'static>] = &[
    ProfileDescriptor {
        profile_id: "gtm",
        jobs: GTM_JOBS,
        input_adapter: Some(AdapterKind::GtmProspect),
        template_id: "gtm",
    },
    ProfileDescriptor {
        profile_id: "proposal",
        jobs: PROPOSAL_JOBS,
        input_adapter: Some(AdapterKind::ProposalOpportunity),
        template_id: "proposal",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegistryError {
    DuplicateProfile(&'static str),
    DuplicateJob(&'static str),
    CrossProfileJob(&'static str),
    UnknownSkill(&'static str),
    MissingAdapter(&'static str),
    AmbiguousAdapter(&'static str),
    DuplicateTemplate(&'static str),
}

pub(crate) fn validate_registry(
    descriptors: &[ProfileDescriptor<'_>],
) -> Result<(), RegistryError> {
    let mut profiles = std::collections::BTreeSet::new();
    let mut jobs = std::collections::BTreeMap::new();
    let mut adapters = std::collections::BTreeMap::new();
    let mut templates = std::collections::BTreeSet::new();
    for descriptor in descriptors {
        if !profiles.insert(descriptor.profile_id) {
            return Err(RegistryError::DuplicateProfile(descriptor.profile_id));
        }
        if !templates.insert(descriptor.template_id) {
            return Err(RegistryError::DuplicateTemplate(descriptor.template_id));
        }
        let adapter = descriptor
            .input_adapter
            .ok_or(RegistryError::MissingAdapter(descriptor.profile_id))?;
        if adapters.insert(adapter, descriptor.profile_id).is_some() {
            return Err(RegistryError::AmbiguousAdapter(descriptor.profile_id));
        }
        for route in descriptor.jobs {
            if !is_packaged_skill(route.skill_id) {
                return Err(RegistryError::UnknownSkill(route.skill_id));
            }
            if let Some(owner) = jobs.insert(route.job_id, descriptor.profile_id) {
                return Err(if owner == descriptor.profile_id {
                    RegistryError::DuplicateJob(route.job_id)
                } else {
                    RegistryError::CrossProfileJob(route.job_id)
                });
            }
        }
    }
    Ok(())
}

fn registry() -> &'static [ProfileDescriptor<'static>] {
    validate_registry(PROFILE_DESCRIPTORS).expect("canonical profile registry must be valid");
    PROFILE_DESCRIPTORS
}
pub(crate) fn profile_descriptor(profile_id: &str) -> Option<&'static ProfileDescriptor<'static>> {
    registry().iter().find(|d| d.profile_id == profile_id)
}
pub(crate) fn route_spec(profile_id: &str, job_id: &str) -> Option<JobRouteSpec> {
    profile_descriptor(profile_id)?
        .jobs
        .iter()
        .copied()
        .find(|r| r.job_id == job_id)
}
pub(crate) fn job_owner(job_id: &str) -> Option<&'static str> {
    registry().iter().find_map(|d| {
        d.jobs
            .iter()
            .any(|r| r.job_id == job_id)
            .then_some(d.profile_id)
    })
}
pub(crate) fn ordered_route_specs() -> impl Iterator<Item = (&'static str, JobRouteSpec)> {
    registry()
        .iter()
        .flat_map(|d| d.jobs.iter().copied().map(move |r| (d.profile_id, r)))
}
pub(crate) fn is_packaged_skill(skill_id: &str) -> bool {
    PACKAGED_SKILL_IDS.contains(&skill_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_registry_is_closed_and_ordered() {
        assert_eq!(
            PROFILE_DESCRIPTORS
                .iter()
                .map(|d| d.profile_id)
                .collect::<Vec<_>>(),
            ["gtm", "proposal"]
        );
        assert_eq!(
            ordered_route_specs()
                .map(|(_, r)| r.job_id)
                .collect::<Vec<_>>(),
            [
                "prospect-fit-or-brief",
                "outbound-copy-brief",
                "outbound-copy-review",
                "bid-no-bid-review",
                "compliance-review",
                "proof-review",
                "red-team-review"
            ]
        );
        assert!(validate_registry(PROFILE_DESCRIPTORS).is_ok());
    }
    #[test]
    fn registry_rejects_conflicts_and_unknowns() {
        assert!(matches!(
            validate_registry(&[PROFILE_DESCRIPTORS[0], PROFILE_DESCRIPTORS[0]]),
            Err(RegistryError::DuplicateProfile(_))
        ));

        const DUPLICATE_JOBS: &[JobRouteSpec] = &[
            JobRouteSpec {
                job_id: "duplicate",
                skill_id: "mdp-pack-apply",
            },
            JobRouteSpec {
                job_id: "duplicate",
                skill_id: "mdp-pack-apply",
            },
        ];
        let mut duplicate = PROFILE_DESCRIPTORS[0];
        duplicate.jobs = DUPLICATE_JOBS;
        assert!(matches!(
            validate_registry(&[duplicate]),
            Err(RegistryError::DuplicateJob("duplicate"))
        ));

        let mut cross_profile = PROFILE_DESCRIPTORS[1];
        const CROSS_PROFILE_JOBS: &[JobRouteSpec] = &[GTM_JOBS[0]];
        cross_profile.jobs = CROSS_PROFILE_JOBS;
        assert!(matches!(
            validate_registry(&[PROFILE_DESCRIPTORS[0], cross_profile]),
            Err(RegistryError::CrossProfileJob("prospect-fit-or-brief"))
        ));

        let mut bad = PROFILE_DESCRIPTORS[0];
        const UNKNOWN_SKILL_JOBS: &[JobRouteSpec] = &[JobRouteSpec {
            job_id: "x",
            skill_id: "not-packaged",
        }];
        bad.jobs = UNKNOWN_SKILL_JOBS;
        assert!(matches!(
            validate_registry(&[bad]),
            Err(RegistryError::UnknownSkill(_))
        ));
        let mut missing = PROFILE_DESCRIPTORS[0];
        missing.input_adapter = None;
        assert!(matches!(
            validate_registry(&[missing]),
            Err(RegistryError::MissingAdapter(_))
        ));

        let mut ambiguous = PROFILE_DESCRIPTORS[1];
        ambiguous.input_adapter = PROFILE_DESCRIPTORS[0].input_adapter;
        assert!(matches!(
            validate_registry(&[PROFILE_DESCRIPTORS[0], ambiguous]),
            Err(RegistryError::AmbiguousAdapter("proposal"))
        ));

        let mut duplicate_template = PROFILE_DESCRIPTORS[1];
        duplicate_template.template_id = PROFILE_DESCRIPTORS[0].template_id;
        assert!(matches!(
            validate_registry(&[PROFILE_DESCRIPTORS[0], duplicate_template]),
            Err(RegistryError::DuplicateTemplate("gtm"))
        ));
    }

    #[test]
    fn lookups_fail_closed_without_job_inference() {
        assert!(profile_descriptor("support").is_none());
        assert!(route_spec("proposal", "prospect-fit-or-brief").is_none());
        assert_eq!(job_owner("prospect-fit-or-brief"), Some("gtm"));
        assert!(route_spec("gtm", "not-a-job").is_none());
    }
}
