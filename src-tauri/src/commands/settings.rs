use crate::db::with_conn;
use crate::error::CommandResult;
use crate::state::AppState;
use crate::types::AppSettings;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    with_conn(&state.db, |conn| Ok(crate::db::read_settings(conn))).await
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> CommandResult<()> {
    let json = serde_json::to_string(&settings)?;
    with_conn(&state.db, move |conn| {
        conn.execute(
            "INSERT INTO settings (id, payload) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            [json],
        )?;
        Ok(())
    })
    .await
}
