use std::{collections::HashSet, thread, time};

use radicle::crypto::{test::signer::MockSigner, Signer};
use radicle::git;
use radicle::node::{FetchResult, Handle as _};
use radicle::storage::{ReadRepository, ReadStorage, WriteRepository, WriteStorage};
use radicle::test::fixtures;
use radicle::{assert_matches, rad};
use service::Config;

use crate::service;
use crate::service::config::Limits;
use crate::service::tracking::Scope;
use crate::storage::git::transport;
use crate::test::environment::{converge, Environment, Node};
use crate::test::logger;

#[test]
//
//     alice -- bob
//
fn test_inventory_sync_basic() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();

    let mut alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());

    alice.project("alice", "");
    bob.project("bob", "");

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());

    alice.connect(&bob);

    let routes = converge([&alice, &bob]);
    assert_eq!(routes.len(), 2);
}

#[test]
//
//     alice -- bob -- eve
//
fn test_inventory_sync_bridge() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();

    let mut alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let mut eve = Node::init(tmp.path());

    alice.project("alice", "");
    bob.project("bob", "");
    eve.project("eve", "");

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());
    let eve = eve.spawn(service::Config::default());

    alice.connect(&bob);
    bob.connect(&eve);

    let routes = converge([&alice, &bob, &eve]);
    assert_eq!(routes.len(), 3);
}

#[test]
//
//     alice -- bob
//       |       |
//     carol -- eve
//
fn test_inventory_sync_ring() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();

    let mut alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let mut eve = Node::init(tmp.path());
    let mut carol = Node::init(tmp.path());

    alice.project("alice", "");
    bob.project("bob", "");
    eve.project("eve", "");
    carol.project("carol", "");

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());
    let mut eve = eve.spawn(service::Config::default());
    let mut carol = carol.spawn(service::Config::default());

    alice.connect(&bob);
    bob.connect(&eve);
    eve.connect(&carol);
    carol.connect(&alice);

    let routes = converge([&alice, &bob, &eve, &carol]);
    assert_eq!(routes.len(), 4);
}

#[test]
//
//             dave
//              |
//     eve -- alice -- bob
//              |
//            carol
//
fn test_inventory_sync_star() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();

    let mut alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let mut eve = Node::init(tmp.path());
    let mut carol = Node::init(tmp.path());
    let mut dave = Node::init(tmp.path());

    alice.project("alice", "");
    bob.project("bob", "");
    eve.project("eve", "");
    carol.project("carol", "");
    dave.project("dave", "");

    let alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());
    let mut eve = eve.spawn(service::Config::default());
    let mut carol = carol.spawn(service::Config::default());
    let mut dave = dave.spawn(service::Config::default());

    bob.connect(&alice);
    eve.connect(&alice);
    carol.connect(&alice);
    dave.connect(&alice);

    let routes = converge([&alice, &bob, &eve, &carol, &dave]);
    assert_eq!(routes.len(), 5);
}

#[test]
fn test_replication() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let acme = bob.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    let inventory = alice.storage.inventory().unwrap();
    assert!(inventory.is_empty());

    let tracked = alice.handle.track_repo(acme, Scope::All).unwrap();
    assert!(tracked);

    let seeds = alice.handle.seeds(acme).unwrap();
    assert!(seeds.is_connected(&bob.id));

    let result = alice.handle.fetch(acme, bob.id).unwrap();
    assert!(result.is_success());

    let updated = match result {
        FetchResult::Success { updated, .. } => updated,
        FetchResult::Failed { reason } => {
            panic!("Fetch failed from {}: {reason}", bob.id);
        }
    };
    assert!(!updated.is_empty());

    log::debug!(target: "test", "Fetch complete with {}", bob.id);

    let inventory = alice.storage.inventory().unwrap();
    let alice_repo = alice.storage.repository(acme).unwrap();
    let bob_repo = bob.storage.repository(acme).unwrap();

    let alice_refs = alice_repo
        .references()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let bob_refs = bob_repo
        .references()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(inventory.first(), Some(&acme));
    assert_eq!(alice_refs, bob_refs);
    assert_matches!(alice.storage.repository(acme).unwrap().validate(), Ok(()));
}

#[test]
fn test_replication_no_delegates() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());

    let acme = bob.project("acme", "");
    // Delete one of the signed refs.
    bob.storage
        .repository_mut(acme)
        .unwrap()
        .reference(&bob.id, &git::qualified!("refs/heads/master"))
        .unwrap()
        .delete()
        .unwrap();

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    alice.handle.track_repo(acme, Scope::All).unwrap();
    let result = alice.handle.fetch(acme, bob.id).unwrap();

    assert_matches!(
        result,
        FetchResult::Failed {
            reason
        } if reason == "no delegates in transfer"
    );
}

#[test]
fn test_replication_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let carol = MockSigner::default();
    let acme = bob.project("acme", "");
    let repo = bob.storage.repository_mut(acme).unwrap();
    let (_, head) = repo.head().unwrap();
    let id = repo.identity_head().unwrap();

    // Create some unsigned refs for Carol in Bob's storage.
    repo.raw()
        .reference(
            &git::qualified!("refs/heads/carol").with_namespace(carol.public_key().into()),
            *head,
            true,
            &String::default(),
        )
        .unwrap();
    repo.raw()
        .reference(
            &git::refs::storage::id(carol.public_key()),
            id.into(),
            true,
            &String::default(),
        )
        .unwrap();

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    alice.handle.track_node(*carol.public_key(), None).unwrap();
    alice.handle.track_repo(acme, Scope::Trusted).unwrap();
    let result = alice.handle.fetch(acme, bob.id).unwrap();

    // Fetch is successful despite not fetching Carol's refs, since she isn't a delegate.
    assert!(result.is_success());

    let repo = alice.storage.repository(acme).unwrap();
    let mut remotes = repo.remote_ids().unwrap();

    assert_eq!(remotes.next().unwrap().unwrap(), bob.id);
    assert!(remotes.next().is_none());

    repo.validate().unwrap();
}

#[test]
fn test_migrated_clone() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let mut alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());
    let acme = alice.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    let tracked = bob.handle.track_repo(acme, Scope::All).unwrap();
    assert!(tracked);

    let result = bob.handle.fetch(acme, alice.id).unwrap();
    assert!(result.is_success());

    log::debug!(target: "test", "Fetch complete with {}", alice.id);

    // Simulate alice deleting the project and cloning it again
    {
        let path = alice.storage.path().join(acme.canonical());
        std::fs::remove_dir_all(path).unwrap();
    }
    assert!(!alice.storage.contains(&acme).unwrap());
    let result = alice.handle.fetch(acme, bob.id).unwrap();
    assert!(result.is_success());

    let alice_repo = alice.storage.repository(acme).unwrap();
    let bob_repo = bob.storage.repository(acme).unwrap();

    let alice_refs = alice_repo
        .references()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let bob_refs = bob_repo
        .references()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(alice_refs, bob_refs);
    assert_matches!(alice.storage.repository(acme).unwrap().validate(), Ok(()));
}

#[test]
fn test_dont_fetch_owned_refs() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let mut alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());
    let acme = alice.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    assert!(bob.handle.track_repo(acme, Scope::Trusted).unwrap());

    let result = bob.handle.fetch(acme, alice.id).unwrap();
    assert!(result.is_success());

    log::debug!(target: "test", "Fetch complete with {}", bob.id);

    alice.issue(acme, "Don't fetch self", "Use ^");
    let result = alice.handle.fetch(acme, bob.id).unwrap();
    assert!(result.is_success())
}

#[test]
fn test_fetch_trusted_remotes() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let mut alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());
    let acme = alice.project("acme", "");
    let mut signers = Vec::with_capacity(5);
    {
        for _ in 0..5 {
            let signer = MockSigner::default();
            rad::fork_remote(acme, &alice.id, &signer, &alice.storage).unwrap();
            signers.push(signer);
        }
    }

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    let trusted = signers
        .iter()
        .map(|s| *s.public_key())
        .take(2)
        .collect::<HashSet<_>>();

    assert!(
        trusted.len() < signers.len(),
        "Bob is only trusting a subset of peers"
    );
    assert!(bob.handle.track_repo(acme, Scope::Trusted).unwrap());
    for nid in &trusted {
        assert!(bob.handle.track_node(*nid, None).unwrap());
    }

    let result = bob.handle.fetch(acme, alice.id).unwrap();
    assert!(result.is_success());

    log::debug!(target: "test", "Fetch complete with {}", bob.id);

    let bob_repo = bob.storage.repository(acme).unwrap();
    let bob_remotes = bob_repo
        .remote_ids()
        .unwrap()
        .collect::<Result<HashSet<_>, _>>()
        .unwrap();

    assert!(bob_remotes.len() == trusted.len() + 1);
    assert!(bob_remotes.is_superset(&trusted));
    assert!(bob_remotes.contains(&alice.id));
}

#[test]
fn test_missing_remote() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let mut alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());
    let acme = alice.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());
    let carol = MockSigner::default();

    alice.connect(&bob);
    converge([&alice, &bob]);

    assert!(bob.handle.track_repo(acme, Scope::Trusted).unwrap());
    assert!(bob.handle.track_node(*carol.public_key(), None).unwrap());
    let result = bob.handle.fetch(acme, alice.id).unwrap();
    assert!(result.is_success());
    log::debug!(target: "test", "Fetch complete with {}", bob.id);
    rad::fork_remote(acme, &alice.id, &carol, &bob.storage).unwrap();

    alice.issue(acme, "Missing Remote", "Fixing the missing remote issue");
    let result = bob.handle.fetch(acme, alice.id).unwrap();
    assert!(result.is_success());
    log::debug!(target: "test", "Fetch complete with {}", bob.id);
}

#[test]
fn test_fetch_preserve_owned_refs() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let mut alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());
    let acme = alice.project("acme", "");
    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    assert!(bob.handle.track_repo(acme, Scope::Trusted).unwrap());
    assert!(bob.handle.track_node(alice.id, None).unwrap());

    let result = bob.handle.fetch(acme, alice.id).unwrap();
    assert!(result.is_success());

    log::debug!(target: "test", "Fetch complete with {}", bob.id);

    alice.issue(acme, "Bug", "Bugs, bugs, bugs");

    let before = alice
        .storage
        .repository(acme)
        .unwrap()
        .references_of(&alice.id)
        .unwrap();

    // Fetch shouldn't prune any of our own refs.
    let result = alice.handle.fetch(acme, bob.id).unwrap();
    let (updated, _) = result.success().unwrap();
    assert_eq!(updated, vec![]);

    let after = alice
        .storage
        .repository(acme)
        .unwrap()
        .references_of(&alice.id)
        .unwrap();

    assert_eq!(before, after);
}

#[test]
fn test_clone() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let acme = bob.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    transport::local::register(alice.storage.clone());

    let _ = alice.handle.track_repo(acme, Scope::All).unwrap();
    let seeds = alice.handle.seeds(acme).unwrap();
    assert!(seeds.is_connected(&bob.id));

    let result = alice.handle.fetch(acme, bob.id).unwrap();
    assert!(result.is_success());

    rad::fork(acme, &alice.signer, &alice.storage).unwrap();

    let working = rad::checkout(
        acme,
        alice.signer.public_key(),
        tmp.path().join("clone"),
        &alice.storage,
    )
    .unwrap();

    // Makes test finish faster.
    drop(alice);

    let head = working.head().unwrap();
    let oid = head.target().unwrap();

    let (_, canonical) = bob
        .storage
        .repository(acme)
        .unwrap()
        .canonical_head()
        .unwrap();

    assert_eq!(oid, *canonical);

    // Make sure that bob has refs/rad/id set
    assert!(bob
        .storage
        .repository(acme)
        .unwrap()
        .identity_head()
        .is_ok());
}

#[test]
fn test_fetch_up_to_date() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let acme = bob.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());

    alice.connect(&bob);
    converge([&alice, &bob]);

    transport::local::register(alice.storage.clone());

    let _ = alice.handle.track_repo(acme, Scope::All).unwrap();
    let result = alice.handle.fetch(acme, bob.id).unwrap();
    assert!(result.is_success());

    // Fetch again! This time, everything's up to date.
    let result = alice.handle.fetch(acme, bob.id).unwrap();
    assert_eq!(
        result.success(),
        Some((vec![], HashSet::from_iter([bob.id])))
    );
}

#[test]
fn test_large_fetch() {
    logger::init(log::Level::Debug);

    let env = Environment::new();
    let scale = env.scale();
    let mut alice = Node::init(&env.tmp());
    let bob = Node::init(&env.tmp());

    let tmp = tempfile::tempdir().unwrap();
    let (repo, _) = fixtures::repository(tmp.path());
    fixtures::populate(&repo, scale.max(3));

    let rid = alice.project_from("acme", "", &repo);

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());
    let bob_events = bob.handle.events();

    bob.handle.track_repo(rid, Scope::All).unwrap();
    alice.connect(&bob);

    bob_events
        .wait(
            |e| matches!(e, service::Event::RefsFetched { updated, .. } if !updated.is_empty()),
            time::Duration::from_secs(9 * scale as u64),
        )
        .unwrap();

    let (_, doc) = bob.storage.repository(rid).unwrap().identity_doc().unwrap();
    let proj = doc.verified().unwrap().project().unwrap();

    assert_eq!(proj.name(), "acme");
}

#[test]
fn test_concurrent_fetches() {
    logger::init(log::Level::Debug);

    let env = Environment::new();
    let scale = env.scale();
    let mut bob_repos = HashSet::new();
    let mut alice_repos = HashSet::new();
    let mut alice = Node::init(&env.tmp());
    let mut bob = Node::init(&env.tmp());
    let repos = scale.max(4);

    for i in 0..repos {
        // Create a repo for Alice.
        let tmp = tempfile::tempdir().unwrap();
        let (repo, _) = fixtures::repository(tmp.path());
        fixtures::populate(&repo, scale);

        let rid = alice.project_from(&format!("alice-{i}"), "", &repo);
        alice_repos.insert(rid);

        // Create a repo for Bob.
        let tmp = tempfile::tempdir().unwrap();
        let (repo, _) = fixtures::repository(tmp.path());
        fixtures::populate(&repo, scale);

        let rid = bob.project_from(&format!("bob-{i}"), "", &repo);
        bob_repos.insert(rid);
    }

    let config = service::Config {
        limits: Limits {
            // Have one fetch be queued.
            fetch_concurrency: repos - 1,
            ..Limits::default()
        },
        ..service::Config::default()
    };
    let mut alice = alice.spawn(config.clone());
    let mut bob = bob.spawn(config);

    let alice_events = alice.handle.events();
    let bob_events = bob.handle.events();

    for rid in &bob_repos {
        alice.handle.track_repo(*rid, Scope::All).unwrap();
    }
    for rid in &alice_repos {
        bob.handle.track_repo(*rid, Scope::All).unwrap();
    }
    alice.connect(&bob);

    while !bob_repos.is_empty() {
        match alice_events.recv().unwrap() {
            service::Event::RefsFetched { rid, updated, .. } if !updated.is_empty() => {
                bob_repos.remove(&rid);
                log::debug!(target: "test", "{} fetched {rid} ({} left)",alice.id, bob_repos.len());
            }
            _ => {}
        }
    }

    while !alice_repos.is_empty() {
        match bob_events.recv().unwrap() {
            service::Event::RefsFetched { rid, updated, .. } if !updated.is_empty() => {
                alice_repos.remove(&rid);
                log::debug!(target: "test", "{} fetched {rid} ({} left)", bob.id, alice_repos.len());
            }
            _ => {}
        }
    }

    for rid in &bob_repos {
        let (_, doc) = alice
            .storage
            .repository(*rid)
            .unwrap()
            .identity_doc()
            .unwrap();
        let proj = doc.verified().unwrap().project().unwrap();

        assert!(proj.name().starts_with("bob"));
    }
    for rid in &alice_repos {
        let (_, doc) = bob
            .storage
            .repository(*rid)
            .unwrap()
            .identity_doc()
            .unwrap();
        let proj = doc.verified().unwrap().project().unwrap();

        assert!(proj.name().starts_with("alice"));
    }
}

#[test]
#[ignore = "failing"]
#[should_panic]
// TODO: This test currently passes but the behavior is wrong. The test should not panic.
// We should figure out why we end up with no sessions established.
fn test_connection_crossing() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();
    let alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());

    alice.handle.connect(bob.id, bob.addr.into()).unwrap();
    bob.handle.connect(alice.id, alice.addr.into()).unwrap();

    thread::sleep(time::Duration::from_secs(1));

    let s1 = alice.handle.sessions().unwrap().contains_key(&bob.id);
    let s2 = bob.handle.sessions().unwrap().contains_key(&alice.id);

    assert!(s1 ^ s2, "Exactly one session should be established");
}

#[test]
/// Alice is going to try to fetch outdated refs of Bob, from Eve. This is a non-fastfoward fetch
/// on the sigrefs branch.
fn test_non_fastforward_sigrefs() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();

    let alice = Node::init(tmp.path());
    let mut bob = Node::init(tmp.path());
    let eve = Node::init(tmp.path());

    let rid = bob.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let bob = bob.spawn(service::Config::default());
    let mut eve = eve.spawn(service::Config::default());

    alice.handle.track_repo(rid, Scope::All).unwrap();
    eve.handle.track_repo(rid, Scope::All).unwrap();

    alice.connect(&bob);
    alice.connect(&eve);
    eve.connect(&bob);

    converge([&alice, &bob, &eve]);

    // Eve fetches the inital project from Bob.
    eve.handle.fetch(rid, bob.id).unwrap();
    // Alice fetches it too.
    alice.handle.fetch(rid, bob.id).unwrap();

    // Now Eve disconnects from Bob so she doesn't fetch his update.
    eve.handle
        .command(service::Command::Disconnect(bob.id))
        .unwrap();

    // Bob updates his refs.
    bob.issue(
        rid,
        "Updated Sigrefs",
        "Updated sigrefs are harshing my vibes",
    );
    // Alice fetches from Bob.
    alice.handle.fetch(rid, bob.id).unwrap();

    // Now Alice has the latest, and when she tries to fetch from Eve, it breaks because
    // Eve has old refs.
    assert_matches!(
        alice.handle.fetch(rid, eve.id).unwrap(),
        FetchResult::Success { .. }
    );
}

#[test]
fn test_outdated_sigrefs() {
    logger::init(log::Level::Debug);

    let tmp = tempfile::tempdir().unwrap();

    let mut alice = Node::init(tmp.path());
    let bob = Node::init(tmp.path());
    let eve = Node::init(tmp.path());

    let rid = alice.project("acme", "");

    let mut alice = alice.spawn(service::Config::default());
    let mut bob = bob.spawn(service::Config::default());
    let mut eve = eve.spawn(service::Config::default());

    bob.handle.track_repo(rid, Scope::All).unwrap();
    eve.handle.track_repo(rid, Scope::All).unwrap();
    alice.connect(&bob);
    bob.connect(&eve);
    eve.connect(&alice);
    converge([&alice, &bob, &eve]);

    bob.handle.fetch(rid, alice.id).unwrap();
    assert!(bob.storage.contains(&rid).unwrap());
    rad::fork(rid, &bob.signer, &bob.storage).unwrap();

    eve.handle.fetch(rid, alice.id).unwrap();
    assert!(eve.storage.contains(&rid).unwrap());
    rad::fork(rid, &eve.signer, &eve.storage).unwrap();

    alice
        .handle
        .track_node(eve.id, Some("eve".to_string()))
        .unwrap();
    alice.handle.fetch(rid, eve.id).unwrap();
    let repo = alice.storage.repository(rid).unwrap();
    assert!(repo.remote(&eve.id).is_ok());

    assert_matches!(
        bob.handle.fetch(rid, eve.id).unwrap(),
        FetchResult::Success { .. }
    );
    let repo = bob.storage.repository(rid).unwrap();
    let eve_remote = repo.remote(&eve.id).unwrap();
    let old_refs = eve_remote.refs;

    // At this stage, Alice and Bob have Eve's fork and Eve does not
    // have Bob's fork

    eve.issue(
        rid,
        "Outdated Sigrefs",
        "Outdated sigrefs are harshing my vibes",
    );
    let repo = eve.storage.repository(rid).unwrap();
    let eves_refs = repo.remote(&eve.id).unwrap().refs;

    // Get the current state of eve's refs in alice's storage
    log::debug!(target: "test", "Alice fetches from Eve..");
    assert_matches!(
        alice.handle.fetch(rid, eve.id).unwrap(),
        FetchResult::Success { .. }
    );
    let repo = alice.storage.repository(rid).unwrap();
    let eve_remote = repo.remote(&eve.id).unwrap();
    let eves_refs_expected = eve_remote.refs;
    assert_ne!(eves_refs_expected, old_refs);
    assert_eq!(eves_refs_expected, eves_refs);

    log::debug!(target: "test", "Alice fetches from Bob..");

    alice
        .handle
        .track_node(bob.id, Some("bob".to_string()))
        .unwrap();
    assert_matches!(
        alice.handle.fetch(rid, bob.id).unwrap(),
        FetchResult::Success { .. }
    );

    // Ensure that Eve's refs have not changed after fetching the old refs from Bob.
    let repo = alice.storage.repository(rid).unwrap();
    let eve_remote = repo.remote(&eve.id).unwrap();
    let eves_refs = eve_remote.refs;

    assert_ne!(eves_refs, old_refs);
    assert_eq!(eves_refs_expected, eves_refs);
}

#[test]
// FIXME: well this is a test, but I do not know exactly what I am looking for.
//
// Hoping just to reproduce my problem.
fn test_fetch_old_refs_for_new_nodes() {
    logger::init(log::Level::Debug);

    let tmp_alice = tempfile::tempdir().unwrap();

    // alice start a very awesome project on the
    // radicle network, so she start a new node
    let mut alice = Node::init(tmp_alice.path());
    let (git_repo, rid) =
        alice.project_with_path("awesome-alice", "an awesome project", tmp_alice.path());
    let alice = alice.spawn(Config::default());

    let project_path = tmp_alice.path().join("awesome-alice");
    // now alice start to feel some issue to manage the
    // work to do on the repository.
    (0..2).for_each(|i| {
        alice.issue(
            rid,
            &format!("{i}: just a title"),
            &format!("{i}: just a description"),
        );
    });

    // FIXME: create a branch before!

    // alice make some change to in a new branch
    fixtures::populate(&git_repo, 2);

    let result = alice.rad("patch", &["open", "--no-message"], project_path);
    assert!(result.is_ok());

    // FIXME: make a patch in some way as I did with the issue.

    let tmp_bob = tempfile::tempdir().unwrap();

    // Now bob see alice very exited talking about this new project
    // and decided to get involved.
    let bob = Node::init(tmp_bob.path());
    let mut bob = bob.spawn(Config::default());
    let result = bob.rad("auth", &[], tmp_bob.path());
    assert!(result.is_ok(), "while running `rad auth` an error occurse");

    // Alice send to node information to bob, and
    // bob now will connect to the alice node
    // and run `rad sync --fetch`.
    bob.connect(&alice);

    // bob clone the information about the repository
    let result = bob.rad("clone", &[rid.to_string().as_str()], tmp_bob.path());
    assert!(
        result.is_ok(),
        "while running `rad clone {rid}` an error occurse {:?}",
        result
    );

    let rpath = format!("{}/awesome-alice", tmp_bob.path().to_str().unwrap());

    let result = bob.rad("sync", &["--fetch"], rpath.clone());
    assert!(
        result.is_ok(),
        "while running `rad fetch --sync` an error occurse: {:?}",
        result
    );

    // Bob that is in sync with the repository
    let result = bob.rad("issue", &[], rpath.clone());
    assert!(
        result.is_ok(),
        "while running `rad list` an error occurse: {:?}",
        result
    i);

    // FIXME: check the patches

    // FIXME: open a couple of issue as bob
    // FIXME: open a new patch as bob

    // FIXME: alice check the patch
    //
    // FIXME: carl clone the repo
    // FIXME: carl check the issue
    // FIXME: carl the patch
}
