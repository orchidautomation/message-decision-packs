pub(crate) const PACKAGED_SKILL_IDS: [&str; 5] = [
    "mdp",
    "mdp-pack-builder",
    "mdp-pack-review",
    "mdp-gtm-brief",
    "mdp-proposal-review",
];

pub(crate) const BOOTSTRAP_SKILL_IDS: [&str; 3] = ["mdp", "mdp-pack-builder", "mdp-pack-review"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JobRouteSpec {
    pub(crate) profile_id: &'static str,
    pub(crate) job_id: &'static str,
    pub(crate) skill_id: &'static str,
}

pub(crate) const JOB_ROUTE_SPECS: [JobRouteSpec; 7] = [
    JobRouteSpec {
        profile_id: "gtm",
        job_id: "prospect-fit-or-brief",
        skill_id: "mdp-gtm-brief",
    },
    JobRouteSpec {
        profile_id: "gtm",
        job_id: "outbound-copy-brief",
        skill_id: "mdp-gtm-brief",
    },
    JobRouteSpec {
        profile_id: "gtm",
        job_id: "outbound-copy-review",
        skill_id: "mdp-gtm-brief",
    },
    JobRouteSpec {
        profile_id: "proposal",
        job_id: "bid-no-bid-review",
        skill_id: "mdp-proposal-review",
    },
    JobRouteSpec {
        profile_id: "proposal",
        job_id: "compliance-review",
        skill_id: "mdp-proposal-review",
    },
    JobRouteSpec {
        profile_id: "proposal",
        job_id: "proof-review",
        skill_id: "mdp-proposal-review",
    },
    JobRouteSpec {
        profile_id: "proposal",
        job_id: "red-team-review",
        skill_id: "mdp-proposal-review",
    },
];

pub(crate) fn is_packaged_skill(skill_id: &str) -> bool {
    PACKAGED_SKILL_IDS.contains(&skill_id)
}

pub(crate) fn route_spec(profile_id: &str, job_id: &str) -> Option<JobRouteSpec> {
    JOB_ROUTE_SPECS
        .iter()
        .copied()
        .find(|route| route.profile_id == profile_id && route.job_id == job_id)
}
