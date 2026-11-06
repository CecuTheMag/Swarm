//! A network's schools (spec 0014): kept, versioned, checked, and only for its members.

use coordinator::{Config, State, accounts, schools::MAX_SCHOOLS_PER_NETWORK};
use protocol::{
    SchoolRecord, SchoolSummary,
    api::{CreatedInvite, NetworkView},
    templates::tt::School,
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN: (&str, &str) = ("admin@swarm.test", "admin password 1");

async fn serve(db: PgPool) -> String {
    accounts::ensure_admin(&db, ADMIN.0, ADMIN.1).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let app = coordinator::router(State::new(db, Config::default()));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// A browser: keeps cookies and sends this site's Origin, as pages do.
#[derive(Clone)]
struct Browser {
    http: Client,
    addr: String,
}

impl Browser {
    fn new(addr: &str) -> Self {
        Self {
            http: Client::builder().cookie_store(true).build().unwrap(),
            addr: addr.to_owned(),
        }
    }

    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> reqwest::Response {
        let mut req = self
            .http
            .request(method, format!("http://{}{path}", self.addr))
            .header("origin", format!("http://{}", self.addr));
        if let Some(body) = body {
            req = req.json(&body);
        }
        req.send().await.unwrap()
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.send(reqwest::Method::GET, path, None).await
    }

    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.send(reqwest::Method::POST, path, Some(body)).await
    }

    async fn put(&self, path: &str, body: Value) -> reqwest::Response {
        self.send(reqwest::Method::PUT, path, Some(body)).await
    }

    async fn delete(&self, path: &str) -> reqwest::Response {
        self.send(reqwest::Method::DELETE, path, None).await
    }

    async fn admin(addr: &str) -> Self {
        let b = Self::new(addr);
        let res = b
            .post(
                "/api/auth/login",
                json!({ "email": ADMIN.0, "password": ADMIN.1 }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        b
    }

    async fn create_network(&self, name: &str) -> Uuid {
        let res = self.post("/api/networks", json!({ "name": name })).await;
        assert_eq!(res.status(), StatusCode::CREATED);
        res.json::<NetworkView>().await.unwrap().id
    }

    /// Someone with a new account, invited into `network` with `role`.
    async fn invite(&self, network: Uuid, role: &str, email: &str) -> Browser {
        let res = self
            .post(
                &format!("/api/networks/{network}/invites"),
                json!({ "role": role }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        let invite: CreatedInvite = res.json().await.unwrap();
        let them = Browser::new(&self.addr);
        let res = them
            .post(
                &format!("/api/invites/{}", invite.token),
                json!({ "email": email, "name": "Петя Колева", "password": "member password" }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        them
    }

    async fn create(&self, network: Uuid, school: &School) -> reqwest::Response {
        self.post(
            &format!("/api/networks/{network}/schools"),
            json!({ "school": school }),
        )
        .await
    }

    async fn save(
        &self,
        network: Uuid,
        id: Uuid,
        school: &School,
        version: u32,
    ) -> reqwest::Response {
        self.put(
            &format!("/api/networks/{network}/schools/{id}"),
            json!({ "school": school, "version": version }),
        )
        .await
    }
}

fn school(classes: usize) -> School {
    timetable::sample::school(3, classes)
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_school_is_kept_as_saved_and_two_editors_never_overwrite_each_other(db: PgPool) {
    let addr = serve(db).await;
    let admin = Browser::admin(&addr).await;
    let net = admin.create_network("ПМГ").await;

    let res = admin.create(net, &school(4)).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: SchoolRecord = res.json().await.unwrap();
    assert_eq!(created.school, school(4));
    let s = &created.summary;
    assert_eq!((s.version, s.classes, s.hours), (1, 4, 4 * 31));
    assert_eq!(s.name, school(4).name);
    assert_eq!(s.teachers as usize, school(4).teachers.len());
    let path = format!("/api/networks/{net}/schools/{}", s.id);
    let read: SchoolRecord = admin.get(&path).await.json().await.unwrap();
    assert_eq!(read, created);

    // Two admins open version 1; the first save wins, the second is told.
    let mut first = school(4);
    first.name = "ПМГ, I срок".into();
    first.teachers[0].unavailable = vec![timetable::Slot { day: 2, period: 0 }];
    let res = admin.save(net, s.id, &first, 1).await;
    assert_eq!(res.status(), StatusCode::OK);
    let saved: SchoolRecord = res.json().await.unwrap();
    assert_eq!(
        (saved.summary.version, saved.school.clone()),
        (2, first.clone())
    );
    assert_eq!(saved.summary.name, "ПМГ, I срок");
    assert_eq!(saved.summary.updated_by.as_deref(), Some("admin"));

    let other = admin.invite(net, "admin", "deputy@school.bg").await;
    let res = other.save(net, s.id, &school(5), 1).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let msg = res.text().await.unwrap();
    assert!(msg.contains("version 2"), "{msg}");
    // Nothing of the refused save was kept.
    let read: SchoolRecord = admin.get(&path).await.json().await.unwrap();
    assert_eq!((read.summary.version, read.school), (2, first));

    // Saved from the newest version, it goes through, by the other admin.
    let res = other.save(net, s.id, &school(5), 2).await;
    assert_eq!(res.status(), StatusCode::OK);
    let saved: SchoolRecord = res.json().await.unwrap();
    assert_eq!(saved.summary.version, 3);
    assert_eq!(saved.summary.updated_by.as_deref(), Some("Петя Колева"));

    // A save must say what it was edited from; a create must not.
    let res = admin.put(&path, json!({ "school": school(5) })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = admin
        .post(
            &format!("/api/networks/{net}/schools"),
            json!({ "school": school(5), "version": 3 }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Listed newest first, without the data.
    admin.create(net, &school(1)).await;
    let list: Vec<SchoolSummary> = admin
        .get(&format!("/api/networks/{net}/schools"))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].classes, 1);
    assert_eq!(list[1].id, s.id);

    let res = admin.delete(&path).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(admin.get(&path).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(admin.delete(&path).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        admin.save(net, s.id, &school(5), 3).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn members_read_schools_admins_change_them_and_other_networks_see_nothing(db: PgPool) {
    let addr = serve(db.clone()).await;
    let admin = Browser::admin(&addr).await;
    let net = admin.create_network("ПМГ").await;
    let created: SchoolRecord = admin.create(net, &school(2)).await.json().await.unwrap();
    let path = format!("/api/networks/{net}/schools/{}", created.summary.id);

    let viewer = admin.invite(net, "viewer", "teacher@school.bg").await;
    assert_eq!(viewer.get(&path).await.status(), StatusCode::OK);
    assert_eq!(
        viewer
            .get(&format!("/api/networks/{net}/schools"))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        viewer.create(net, &school(2)).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        viewer
            .save(net, created.summary.id, &school(3), 1)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(viewer.delete(&path).await.status(), StatusCode::FORBIDDEN);

    // An admin of another network: not even told the school exists.
    let other_net = admin.create_network("Друго училище").await;
    let outsider = admin.invite(other_net, "owner", "other@school.bg").await;
    assert_eq!(outsider.get(&path).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        outsider
            .save(net, created.summary.id, &school(3), 1)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    // Nor through its own network's address.
    let mixed = format!("/api/networks/{other_net}/schools/{}", created.summary.id);
    assert_eq!(outsider.get(&mixed).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        outsider.delete(&mixed).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        outsider
            .save(other_net, created.summary.id, &school(3), 1)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    // Without signing in, nothing.
    let anonymous = Browser::new(&addr);
    assert_eq!(
        anonymous.get(&path).await.status(),
        StatusCode::UNAUTHORIZED
    );

    // Deleting the network deletes its schools.
    let res = admin.delete(&format!("/api/networks/{net}")).await;
    assert!(res.status().is_success(), "{}", res.status());
    let (left,): (i64,) = sqlx::query_as("SELECT count(*) FROM schools")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn only_schools_a_timetable_can_be_searched_for_are_kept(db: PgPool) {
    let addr = serve(db).await;
    let admin = Browser::admin(&addr).await;
    let net = admin.create_network("ПМГ").await;

    let mut namesakes = school(2);
    namesakes.teachers[1].name = namesakes.teachers[0].name.to_lowercase();
    let mut no_teacher = school(2);
    no_teacher.lessons[0].teacher = 999;
    let mut no_lessons = school(2);
    no_lessons.lessons.clear();
    for (bad, reason) in [
        (&namesakes, "teachers 0 and 1 have the same name"),
        (&no_teacher, "no teacher 999"),
        (&no_lessons, "at least one lesson"),
    ] {
        let res = admin.create(net, bad).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let msg = res.text().await.unwrap();
        assert!(msg.contains(reason), "{msg}");
    }
    let created: SchoolRecord = admin.create(net, &school(2)).await.json().await.unwrap();
    let res = admin.save(net, created.summary.id, &namesakes, 1).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Not a school at all.
    let res = admin
        .post(
            &format!("/api/networks/{net}/schools"),
            json!({ "school": { "name": "x" } }),
        )
        .await;
    assert!(res.status().is_client_error());
    // Larger than any school: refused before it is read.
    let huge = json!({ "school": { "name": "x".repeat(4 << 20) } });
    let res = admin
        .post(&format!("/api/networks/{net}/schools"), huge)
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_network_keeps_at_most_fifty_schools_even_when_created_at_once(db: PgPool) {
    let addr = serve(db).await;
    let admin = Browser::admin(&addr).await;
    let net = admin.create_network("ПМГ").await;
    let small = school(1);
    for _ in 0..MAX_SCHOOLS_PER_NETWORK - 5 {
        assert_eq!(
            admin.create(net, &small).await.status(),
            StatusCode::CREATED
        );
    }
    // Twenty at once, for the last five places.
    let attempts = (0..20).map(|_| {
        let (admin, small) = (admin.clone(), small.clone());
        tokio::spawn(async move { admin.create(net, &small).await.status() })
    });
    let mut statuses = Vec::new();
    for a in attempts {
        statuses.push(a.await.unwrap());
    }
    let created = statuses
        .iter()
        .filter(|s| **s == StatusCode::CREATED)
        .count();
    let refused = statuses
        .iter()
        .filter(|s| **s == StatusCode::CONFLICT)
        .count();
    assert_eq!((created, refused), (5, 15), "{statuses:?}");
    let list: Vec<SchoolSummary> = admin
        .get(&format!("/api/networks/{net}/schools"))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(list.len() as i64, MAX_SCHOOLS_PER_NETWORK);
    // Another network has its own fifty.
    let other = admin.create_network("Друго").await;
    assert_eq!(
        admin.create(other, &small).await.status(),
        StatusCode::CREATED
    );
}
