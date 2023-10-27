pub use cluster::Cluster;

mod cluster;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{DynamicObject, ListParams, Patch, PatchParams};
use kube::core::crd::merge_crds;
use kube::{Api, Client, CustomResourceExt, Resource};
use tracing::{debug, info};

use crate::version::ApiVersion;
use crate::wait_crd_established;

const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/crd";

/// Setup CRD
pub(super) async fn set_up(
    kube_client: &Client,
    manage_crd: bool,
    auto_migration: bool,
) -> anyhow::Result<()> {
    if !manage_crd {
        info!("--manage-crd set to false, skip checking CRD");
        return Ok(());
    }

    let crd_api: Api<CustomResourceDefinition> = Api::all(kube_client.clone());
    let definition = Cluster::crd();
    let current_version: ApiVersion<Cluster> = Cluster::version(&()).as_ref().parse()?;

    let ret = crd_api.get(Cluster::crd_name()).await;
    if let Err(kube::Error::Api(kube::error::ErrorResponse { code: 404, .. })) = ret {
        // the following code needs `customresourcedefinitions` write permission
        debug!("cannot found XlineCluster CRD, try to init it");
        _ = crd_api
            .patch(
                Cluster::crd_name(),
                &PatchParams::apply(FIELD_MANAGER),
                &Patch::Apply(definition.clone()),
            )
            .await?;
        wait_crd_established(crd_api.clone(), Cluster::crd_name()).await?;
        return Ok(());
    }

    debug!("found XlineCluster CRD, current version: {current_version}");

    let mut add = true;
    let mut storage = String::new();

    let mut crds = ret?
        .spec
        .versions
        .iter()
        .cloned()
        .map(|ver| {
            let mut crd = definition.clone();
            if ver.name == current_version.to_string() {
                add = false;
            }
            if ver.storage {
                storage = ver.name.clone();
            }
            crd.spec.versions = vec![ver];
            crd
        })
        .collect::<Vec<_>>();

    if add {
        crds.push(definition.clone());
    } else {
        debug!("current version already exists, try to migrate");
        try_migration(
            kube_client,
            crds,
            &current_version,
            &storage,
            auto_migration,
        )
        .await?;
        return Ok(());
    }

    let merged_crd = merge_crds(crds.clone(), &storage)?;
    debug!("try to update crd");
    _ = crd_api
        .patch(
            Cluster::crd_name(),
            &PatchParams::apply(FIELD_MANAGER),
            &Patch::Apply(merged_crd),
        )
        .await?;
    wait_crd_established(crd_api.clone(), Cluster::crd_name()).await?;

    debug!("crd updated, try to migrate");
    try_migration(
        kube_client,
        crds,
        &current_version,
        &storage,
        auto_migration,
    )
    .await?;

    Ok(())
}

/// Try to migrate CRD
#[allow(clippy::indexing_slicing)] // there is at least one element in `versions`
#[allow(clippy::expect_used)]
async fn try_migration(
    kube_client: &Client,
    crds: Vec<CustomResourceDefinition>,
    current_version: &ApiVersion<Cluster>,
    storage: &str,
    auto_migration: bool,
) -> anyhow::Result<()> {
    if !auto_migration {
        debug!("auto migration is disabled, skip migration");
        return Ok(());
    }
    if current_version.to_string() == storage {
        // stop migration if current version is already in storage
        debug!("current version is already in storage, skip migration");
        return Ok(());
    }
    let versions: Vec<ApiVersion<Cluster>> = crds
        .iter()
        .map(|crd| crd.spec.versions[0].name.parse())
        .collect::<anyhow::Result<_>>()?;
    if versions.iter().any(|ver| current_version < ver) {
        // stop migration if current version is less than any version in `versions`
        debug!("current version is less than some version in crd, skip migration");
        return Ok(());
    }
    let group = kube::discovery::group(kube_client, Cluster::group(&()).as_ref()).await?;
    let Some((ar, _)) = group
        .versioned_resources(storage)
        .into_iter()
        .find(|res| res.0.kind == Cluster::kind(&())) else { return Ok(()) };
    let api: Api<DynamicObject> = Api::all_with(kube_client.clone(), &ar);
    let clusters = api.list(&ListParams::default()).await?.items;
    if !clusters.is_empty() && !current_version.compat_with(&storage.parse()?) {
        // there is some clusters with storage version and is not compat with current version, stop migration
        // TODO add a flag to these clusters to indicate that they need to be migrated
        return Ok(());
    }
    // start migration as there is no cluster with storage version
    let merged_crd = merge_crds(crds, &current_version.to_string())?;
    let crd_api: Api<CustomResourceDefinition> = Api::all(kube_client.clone());
    debug!("try to migrate crd from {storage} to {current_version}");
    _ = crd_api
        .patch(
            Cluster::crd_name(),
            &PatchParams::apply(FIELD_MANAGER),
            &Patch::Apply(merged_crd),
        )
        .await?;
    wait_crd_established(crd_api.clone(), Cluster::crd_name()).await?;
    Ok(())
}
