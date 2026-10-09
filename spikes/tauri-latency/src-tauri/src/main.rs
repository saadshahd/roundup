use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{ipc::{Channel, InvokeResponseBody}, State};

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as f64 / 1000.0
}

struct Pty {
    writer: Mutex<Box<dyn Write + Send>>,
    // (t_recv, t_written) per write, FIFO; matched to echo reads (serial driver => 1:1)
    pending: std::sync::Arc<Mutex<VecDeque<(f64, f64)>>>,
    _master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
}

// Output frame: 4 x f64 LE (t_recv, t_written, t_read, t_sent; epoch ms) then echoed bytes.
#[tauri::command]
fn pty_start(state: State<Mutex<Option<Pty>>>, on_out: Channel<InvokeResponseBody>) -> Result<(), String> {
    let pair = native_pty_system()
        .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;
    // raw, no tty echo: bytes only come back if `cat` itself echoes them => true PTY round trip
    let mut cmd = CommandBuilder::new("sh");
    cmd.args(["-c", "stty raw -echo; exec cat"]);
    let _child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let pending = std::sync::Arc::new(Mutex::new(VecDeque::new()));
    let p2 = pending.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 { break; }
            let t_read = now_ms();
            let (t_recv, t_written) = p2.lock().unwrap().pop_front().unwrap_or((f64::NAN, f64::NAN));
            let mut out = Vec::with_capacity(32 + n);
            let t_sent_idx = out.len() + 24;
            for v in [t_recv, t_written, t_read, 0.0] { out.extend_from_slice(&v.to_le_bytes()); }
            out.extend_from_slice(&buf[..n]);
            out[t_sent_idx..t_sent_idx + 8].copy_from_slice(&now_ms().to_le_bytes());
            let _ = on_out.send(InvokeResponseBody::Raw(out));
        }
    });
    *state.lock().unwrap() = Some(Pty { writer: Mutex::new(writer), pending, _master: Mutex::new(pair.master) });
    Ok(())
}

#[tauri::command]
fn pty_write(state: State<Mutex<Option<Pty>>>, data: String) -> Result<(), String> {
    let t_recv = now_ms();
    let g = state.lock().unwrap();
    let pty = g.as_ref().ok_or("no pty")?;
    let mut w = pty.writer.lock().unwrap();
    // hold the queue lock across the write so the reader can't pop a half-filled record
    let mut q = pty.pending.lock().unwrap();
    q.push_back((t_recv, 0.0));
    w.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())?;
    q.back_mut().unwrap().1 = now_ms();
    Ok(())
}

#[tauri::command]
fn save_results(path: String, json: String) -> Result<(), String> {
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn quit(app: tauri::AppHandle) { app.exit(0) }

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(None::<Pty>))
        .invoke_handler(tauri::generate_handler![pty_start, pty_write, save_results, quit])
        .run(tauri::generate_context!())
        .expect("run");
}
