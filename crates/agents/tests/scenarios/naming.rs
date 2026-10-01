//! A9 a first prompt names its Agent.

use contracts::agent::RailNode;
use serde_json::json;

use crate::common::Fixture;

impl Fixture {
    async fn submit(&self, id: &str, prompt: &str) {
        let payload = json!({"hook_event_name": "UserPromptSubmit", "prompt": prompt});
        self.call("agent.signal", json!({"id": id, "payload": payload}))
            .await
            .unwrap();
    }

    async fn name_of(&self, id: &str) -> String {
        let tree: Vec<RailNode> = self.tree().await;
        tree.into_iter().find(|n| n.id == id).unwrap().name
    }
}

#[tokio::test]
async fn a9_an_agent_is_new_agent_until_its_first_prompt_names_it() {
    let mut f = Fixture::running("sleep 30");
    let node = f
        .spawn(None, Some("Fix the refresh race in token.ts"))
        .await
        .unwrap();
    assert_eq!(node.name, "new-agent");
    f.changed();

    f.submit(&node.id, "Fix the refresh race in token.ts").await;

    assert_eq!(f.name_of(&node.id).await, "fix-refresh-race");
    assert_eq!(f.changed(), 1);
}

#[tokio::test]
async fn a9_later_prompts_never_rename_the_agent() {
    let mut f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.submit(&node.id, "fix the build").await;
    f.changed();

    f.submit(&node.id, "now write the docs").await;

    assert_eq!(f.name_of(&node.id).await, "fix-build");
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a9_a_prompt_with_no_words_left_keeps_the_name_and_still_counts_as_the_first() {
    let mut f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.changed();

    f.submit(&node.id, "please, the...").await;
    f.submit(&node.id, "fix the build").await;

    assert_eq!(f.name_of(&node.id).await, "new-agent");
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a9_a_rename_before_the_first_prompt_wins() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.call("rail.rename", json!({"id": node.id, "name": "mine"}))
        .await
        .unwrap();

    f.submit(&node.id, "fix the build").await;

    assert_eq!(f.name_of(&node.id).await, "mine");
}

#[tokio::test]
async fn a9_a_meta_agent_keeps_its_groups_name() {
    let f = Fixture::running("sleep 30");
    let team = f.group("team", None).await;
    f.call("rail.promote", json!({"id": team})).await.unwrap();

    f.submit(&team, "fix the build").await;

    assert_eq!(f.name_of(&team).await, "team");
}
