use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use ureq;

// use crate::package::Package;
// use crate::request_client::UreqClientLive;

//------------------------------------------------------------------------------
// see https://google.github.io/osv.dev/post-v1-querybatch/

// OSV request component
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OSVPackage {
    name: String,
    ecosystem: String,
}

/// OSV request component
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OSVPackageQuery {
    package: OSVPackage,
    version: String,
    // note: commit can go here
}

/// OSV request component
#[derive(Serialize, Deserialize, Debug)]
struct OSVQueryBatch {
    queries: Vec<OSVPackageQuery>,
}

/// OSV response component
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OSVVuln {
    id: String,
    modified: String,
}

/// OSV response component
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OSVQueryResult {
    vulns: Option<Vec<OSVVuln>>,
}

/// OSV response component
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OSVResponse {
    results: Vec<OSVQueryResult>,
}


//------------------------------------------------------------------------------

// Function to send a single batch of queries to the OSV API
fn query_osv_batch<U: UreqClient + std::marker::Sync>(
    client: &U,
    packages: &[OSVPackageQuery],
) -> Vec<Option<Vec<String>>> {
    let url = "https://api.osv.dev/v1/querybatch";

    let batch_query = OSVQueryBatch {
        queries: packages.to_vec(),
    };
    let body = serde_json::to_string(&batch_query).unwrap();
    println!("{:?}", body);

    let response: Result<String, ureq::Error> = client.post(url, &body);
    match response {
        Ok(body_str) => {
            // let body_str = body.into_string().unwrap_or_default();
            // println!("{:?}", body_str);
            let osv_res: OSVResponse = serde_json::from_str(&body_str).unwrap();

            osv_res
                .results
                .iter()
                .map(|result| {
                    result.vulns.as_ref().map(|vuln_list| {
                        vuln_list
                            .iter()
                            .map(|v| v.id.clone())
                            .collect::<Vec<String>>()
                    })
                })
                .collect()
        }
        Err(_) => {
            vec![None; packages.len()]
        }
    }
}

fn query_osv<U: UreqClient + std::marker::Sync>(
    client: &U,
    packages: Vec<OSVPackageQuery>,
) -> Vec<Option<Vec<String>>> {
    // par_chunks sends groups of 4 to batch query
    let results: Vec<Option<Vec<String>>> = packages
        .par_chunks(4)
        .flat_map(|chunk| query_osv_batch(client, chunk))
        .collect();
    results
}

//--------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ureq_client::UreqClientMock;

    #[test]
    fn test_osv_querybatch_a() {
        let client_mock = UreqClientMock {
            mock_response : "{\"results\":[{\"vulns\":[{\"id\":\"GHSA-34rf-p3r3-58x2\",\"modified\":\"2024-05-06T14:46:47.572046Z\"},{\"id\":\"GHSA-3f95-mxq2-2f63\",\"modified\":\"2024-04-10T22:19:39.095481Z\"},{\"id\":\"GHSA-48cq-79qq-6f7x\",\"modified\":\"2024-05-21T14:58:25.710902Z\"}]},{\"vulns\":[{\"id\":\"GHSA-pmv9-3xqp-8w42\",\"modified\":\"2024-09-18T19:36:03.377591Z\"}]}]}".to_string(),
        };

        let packages = vec![
            OSVPackageQuery {
                package: OSVPackage {
                    name: "gradio".to_string(),
                    ecosystem: "PyPI".to_string(),
                },
                version: "4.0.0".to_string(),
            },
            OSVPackageQuery {
                package: OSVPackage {
                    name: "mesop".to_string(),
                    ecosystem: "PyPI".to_string(),
                },
                version: "0.11.1".to_string(),
            },
        ];

        let results = query_osv(&client_mock, packages.clone());

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(vec!["GHSA-34rf-p3r3-58x2".to_string(), "GHSA-3f95-mxq2-2f63".to_string(), "GHSA-48cq-79qq-6f7x".to_string()]));
        assert_eq!(results[1], Some(vec!["GHSA-pmv9-3xqp-8w42".to_string()]));
    }
}

// NOTE: this works
// cat <<EOF | curl -d @- "https://api.osv.dev/v1/querybatch"
// {"queries":[{"package":{"name":"gradio","ecosystem":"PyPI"},"version":"4.0.0"},{"package":{"name":"mesop","ecosystem":"PyPI"},"version":"0.11.1"}]}
// EOF
