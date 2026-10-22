// This code is part of Qiskit.
//
// (C) Copyright IBM 2026
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

//! The environment a bound claim gives its workload.
//!
//! It mirrors what the QRMI SPANK plugin sets for a Slurm job, so a program
//! that finds its resource with `QRMIService().resources()` runs unchanged
//! under either scheduler:
//!
//! - `QRMI_JOB_QPU_RESOURCES` / `QRMI_JOB_QPU_TYPES`, and their legacy
//!   `SLURM_JOB_QPU_*` names;
//! - `QRMI_JOB_ID` and, when known, `QRMI_JOB_UID`;
//! - `<resource_id>_<key>` for each resolved config and credential variable;
//! - `<resource_id>_QRMI_JOB_ACQUISITION_TOKEN` and
//!   `<resource_id>_QRMI_JOB_TIMEOUT_SECONDS`.
//!
//! A pod gets one resource: if it reads several claims' Secrets through
//! `envFrom`, the last Secret's `QRMI_JOB_QPU_*` values win.

use std::collections::{BTreeMap, HashMap};

use k8s_openapi::api::batch::v1::Job;

use crate::crd::{QrmiResourceType, QuantumResourceSpec};

/// What SPANK sets when a job has no time limit.
pub const NO_TIMEOUT_SECONDS: u64 = u32::MAX as u64;

/// The workload a claim serves: what SPANK takes from the Slurm job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workload {
    /// `QRMI_JOB_ID`: the owning Job's UID, else the claim's.
    pub id: String,
    /// `QRMI_JOB_UID`: the pod's `runAsUser`, when the Job sets one.
    pub uid: Option<i64>,
    /// `<resource_id>_QRMI_JOB_TIMEOUT_SECONDS`.
    pub timeout_seconds: u64,
}

/// Name of a per-resource variable, as SPANK builds it: the resource ID as is.
pub fn env_name(resource_id: &str, key: &str) -> String {
    format!("{resource_id}_{key}")
}

/// The QRMI resource type, whose name goes into `QRMI_JOB_QPU_TYPES`.
pub fn qrmi_type(resource_type: &QrmiResourceType) -> qrmi::models::ResourceType {
    use qrmi::models::ResourceType;
    match resource_type {
        QrmiResourceType::IbmQuantumSystem => ResourceType::IBMQuantumSystem,
        QrmiResourceType::QiskitRuntimeService => ResourceType::QiskitRuntimeService,
        QrmiResourceType::PasqalCloud => ResourceType::PasqalCloud,
        QrmiResourceType::PasqalLocal => ResourceType::PasqalLocal,
        QrmiResourceType::AliceBobFelis => ResourceType::AliceBobFelis,
        QrmiResourceType::IqmServer => ResourceType::IQMServer,
    }
}

/// Describe the workload of a claim with UID `claim_uid` and TTL `ttl`,
/// owned by `owner_job` when the claim came from an annotated Job.
pub fn workload_for(claim_uid: &str, ttl: Option<u32>, owner_job: Option<&Job>) -> Workload {
    let job_spec = owner_job.and_then(|job| job.spec.as_ref());
    let pod_spec = job_spec.and_then(|spec| spec.template.spec.as_ref());
    // A container's runAsUser overrides the pod's; QRMI runs in the first container.
    let container_uid = pod_spec
        .and_then(|pod| pod.containers.first())
        .and_then(|c| c.security_context.as_ref())
        .and_then(|sc| sc.run_as_user);
    let pod_uid = pod_spec
        .and_then(|pod| pod.security_context.as_ref())
        .and_then(|sc| sc.run_as_user);
    // Like a Slurm time limit: when the workload will be stopped, whichever
    // comes first of the claim's release and the Job's deadline.
    let deadline = job_spec
        .and_then(|spec| spec.active_deadline_seconds)
        .and_then(|s| u64::try_from(s).ok());
    let timeout_seconds = [ttl.map(u64::from), deadline]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(NO_TIMEOUT_SECONDS);

    Workload {
        id: owner_job
            .and_then(|job| job.metadata.uid.clone())
            .unwrap_or_else(|| claim_uid.to_string()),
        uid: container_uid.or(pod_uid),
        timeout_seconds,
    }
}

/// The job variables, which `acquire()` and `release()` also need.
pub fn job_env(workload: &Workload) -> HashMap<String, String> {
    let mut env = HashMap::from([("QRMI_JOB_ID".to_string(), workload.id.clone())]);
    if let Some(uid) = workload.uid {
        env.insert("QRMI_JOB_UID".to_string(), uid.to_string());
    }
    env
}

/// Recover the job variables from a bound claim's Secret, for `release()`.
pub fn job_env_from_secret(data: &BTreeMap<String, String>) -> HashMap<String, String> {
    ["QRMI_JOB_ID", "QRMI_JOB_UID"]
        .into_iter()
        .filter_map(|key| data.get(key).map(|v| (key.to_string(), v.clone())))
        .collect()
}

/// The Secret data for a bound claim: `resolved` holds the resource's config
/// and credential variables, unprefixed.
pub fn secret_data(
    spec: &QuantumResourceSpec,
    resolved: &HashMap<String, String>,
    token: &str,
    workload: &Workload,
) -> BTreeMap<String, String> {
    let id = &spec.resource_id;
    let type_name = qrmi_type(&spec.resource_type).as_str().to_string();

    let mut data: BTreeMap<String, String> = resolved
        .iter()
        .map(|(k, v)| (env_name(id, k), v.clone()))
        .collect();
    data.insert(
        env_name(id, "QRMI_JOB_ACQUISITION_TOKEN"),
        token.to_string(),
    );
    data.insert(
        env_name(id, "QRMI_JOB_TIMEOUT_SECONDS"),
        workload.timeout_seconds.to_string(),
    );
    for prefix in ["QRMI", "SLURM"] {
        data.insert(format!("{prefix}_JOB_QPU_RESOURCES"), id.clone());
        data.insert(format!("{prefix}_JOB_QPU_TYPES"), type_name.clone());
    }
    data.extend(job_env(workload));
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::batch::v1::JobSpec;
    use k8s_openapi::api::core::v1::{
        Container, PodSecurityContext, PodSpec, PodTemplateSpec, SecurityContext,
    };

    fn felis_spec() -> QuantumResourceSpec {
        QuantumResourceSpec {
            resource_id: "ab_emu_1q_lescanne_2020".to_string(),
            resource_type: QrmiResourceType::AliceBobFelis,
            env_vars: HashMap::new(),
            secret_refs: vec![],
        }
    }

    fn felis_resolved() -> HashMap<String, String> {
        HashMap::from([
            (
                "QRMI_AB_FELIS_BASE_ENDPOINT".to_string(),
                "https://api.alice-bob.com".to_string(),
            ),
            ("QRMI_AB_FELIS_API_KEY".to_string(), "key".to_string()),
        ])
    }

    fn workload(uid: Option<i64>) -> Workload {
        Workload {
            id: "job-uid".to_string(),
            uid,
            timeout_seconds: 3600,
        }
    }

    fn job(
        pod_uid: Option<i64>,
        container_uid: Option<i64>,
        active_deadline_seconds: Option<i64>,
    ) -> Job {
        let mut job = Job::default();
        job.metadata.uid = Some("job-uid".to_string());
        job.spec = Some(JobSpec {
            active_deadline_seconds,
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    security_context: Some(PodSecurityContext {
                        run_as_user: pod_uid,
                        ..Default::default()
                    }),
                    containers: vec![Container {
                        name: "main".to_string(),
                        security_context: Some(SecurityContext {
                            run_as_user: container_uid,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        });
        job
    }

    fn strings(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn secret_data_has_every_variable_spank_sets() {
        let data = secret_data(
            &felis_spec(),
            &felis_resolved(),
            "tok",
            &workload(Some(1000)),
        );

        assert_eq!(
            data,
            strings(&[
                ("QRMI_JOB_ID", "job-uid"),
                ("QRMI_JOB_UID", "1000"),
                ("QRMI_JOB_QPU_RESOURCES", "ab_emu_1q_lescanne_2020"),
                ("QRMI_JOB_QPU_TYPES", "alice-bob-felis"),
                ("SLURM_JOB_QPU_RESOURCES", "ab_emu_1q_lescanne_2020"),
                ("SLURM_JOB_QPU_TYPES", "alice-bob-felis"),
                (
                    "ab_emu_1q_lescanne_2020_QRMI_AB_FELIS_BASE_ENDPOINT", // pragma: allowlist secret
                    "https://api.alice-bob.com"
                ),
                ("ab_emu_1q_lescanne_2020_QRMI_AB_FELIS_API_KEY", "key"),
                ("ab_emu_1q_lescanne_2020_QRMI_JOB_ACQUISITION_TOKEN", "tok"), // pragma: allowlist secret
                ("ab_emu_1q_lescanne_2020_QRMI_JOB_TIMEOUT_SECONDS", "3600"), // pragma: allowlist secret
            ])
        );
    }

    #[test]
    fn secret_data_without_uid_leaves_qrmi_job_uid_out() {
        let data = secret_data(&felis_spec(), &felis_resolved(), "tok", &workload(None));

        assert!(!data.contains_key("QRMI_JOB_UID"));
        assert_eq!(data["QRMI_JOB_ID"], "job-uid");
    }

    #[test]
    fn resource_id_is_the_prefix_unchanged() {
        let spec = QuantumResourceSpec {
            resource_id: "EMU_FREE".to_string(),
            resource_type: QrmiResourceType::PasqalCloud,
            env_vars: HashMap::new(),
            secret_refs: vec![],
        };

        let data = secret_data(&spec, &HashMap::new(), "tok", &workload(None));

        assert_eq!(data["EMU_FREE_QRMI_JOB_ACQUISITION_TOKEN"], "tok");
        assert_eq!(data["QRMI_JOB_QPU_RESOURCES"], "EMU_FREE");
        assert_eq!(data["QRMI_JOB_QPU_TYPES"], "pasqal-cloud");
        assert_eq!(
            env_name("EMU_FREE", "QRMI_JOB_ACQUISITION_TOKEN"),
            "EMU_FREE_QRMI_JOB_ACQUISITION_TOKEN"
        );
    }

    #[test]
    fn type_names_are_the_ones_qrmi_reads() {
        let names: Vec<String> = [
            QrmiResourceType::IbmQuantumSystem,
            QrmiResourceType::QiskitRuntimeService,
            QrmiResourceType::PasqalCloud,
            QrmiResourceType::PasqalLocal,
            QrmiResourceType::AliceBobFelis,
            QrmiResourceType::IqmServer,
        ]
        .iter()
        .map(|t| qrmi_type(t).as_str().to_string())
        .collect();

        assert_eq!(
            names,
            [
                "ibm-quantum-system",
                "qiskit-runtime-service",
                "pasqal-cloud",
                "pasqal-local",
                "alice-bob-felis",
                "iqm-server",
            ]
        );
    }

    #[test]
    fn workload_of_a_job_uses_its_uid_and_run_as_user() {
        let w = workload_for("claim-uid", None, Some(&job(Some(1000), None, None)));

        assert_eq!(
            w,
            Workload {
                id: "job-uid".to_string(),
                uid: Some(1000),
                timeout_seconds: NO_TIMEOUT_SECONDS
            }
        );
    }

    #[test]
    fn container_run_as_user_overrides_the_pod() {
        let w = workload_for("claim-uid", None, Some(&job(Some(1000), Some(2000), None)));

        assert_eq!(w.uid, Some(2000));
    }

    #[test]
    fn timeout_is_the_shorter_of_ttl_and_active_deadline() {
        let deadline_job = job(None, None, Some(600));

        assert_eq!(workload_for("c", Some(3600), None).timeout_seconds, 3600);
        assert_eq!(
            workload_for("c", None, Some(&deadline_job)).timeout_seconds,
            600
        );
        assert_eq!(
            workload_for("c", Some(300), Some(&deadline_job)).timeout_seconds,
            300
        );
        assert_eq!(
            workload_for("c", Some(3600), Some(&deadline_job)).timeout_seconds,
            600
        );
        assert_eq!(
            workload_for("c", None, None).timeout_seconds,
            NO_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn workload_of_a_claim_by_hand_uses_the_claim_uid() {
        let w = workload_for("claim-uid", None, None);

        assert_eq!(
            w,
            Workload {
                id: "claim-uid".to_string(),
                uid: None,
                timeout_seconds: NO_TIMEOUT_SECONDS
            }
        );
    }

    #[test]
    fn job_env_has_id_and_uid_when_known() {
        assert_eq!(
            job_env(&workload(Some(1000))),
            HashMap::from([
                ("QRMI_JOB_ID".to_string(), "job-uid".to_string()),
                ("QRMI_JOB_UID".to_string(), "1000".to_string()),
            ])
        );
        assert_eq!(
            job_env(&workload(None)),
            HashMap::from([("QRMI_JOB_ID".to_string(), "job-uid".to_string())])
        );
    }

    #[test]
    fn job_env_comes_back_from_the_secret() {
        let w = workload(Some(1000));
        let data = secret_data(&felis_spec(), &felis_resolved(), "tok", &w);

        assert_eq!(job_env_from_secret(&data), job_env(&w));
    }
}
