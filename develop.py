#!/usr/bin/env python3
"""
Development script for aaagent-rs chat UI
Manages both frontend (Vite) and backend (Rust) processes
"""

import json
import os
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

# Global flag to stop output threads gracefully
_shutdown_flag = threading.Event()

PID_FILE = ".dev_pids.json"
WEB_DIR = "web"
VITE_PORT = 5173
BACKEND_PORT = 3000


def print_msg(msg, prefix="develop.py"):
    """Print message with prefix"""
    print(f"[{prefix}] {msg}")


def print_success(msg):
    """Print success message"""
    print_msg(f"✓ {msg}")


def print_error(msg):
    """Print error message"""
    print_msg(f"✗ {msg}")


def read_pids():
    """Read PIDs from file"""
    if not os.path.exists(PID_FILE):
        return None
    try:
        with open(PID_FILE, "r") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def write_pids(frontend_pid, backend_pid):
    """Write PIDs to file"""
    data = {
        "frontend": frontend_pid,
        "backend": backend_pid,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    with open(PID_FILE, "w") as f:
        json.dump(data, f, indent=2)


def kill_process(pid):
    """Kill a process by PID (cross-platform)"""
    if pid is None:
        return

    try:
        if sys.platform == "win32":
            # Windows: use taskkill
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(pid)],
                capture_output=True,
                check=False,
            )
        else:
            # Unix: try SIGTERM first, then SIGKILL
            try:
                os.kill(pid, signal.SIGTERM)
                time.sleep(1)
                os.kill(pid, 0)  # Check if still alive
                time.sleep(4)
                os.kill(pid, signal.SIGKILL)  # Force kill
            except ProcessLookupError:
                pass  # Already dead
    except Exception as e:
        print_error(f"Failed to kill process {pid}: {e}")


def stream_output(process, prefix):
    """Stream process output with prefix"""
    try:
        while not _shutdown_flag.is_set():
            line = process.stdout.readline()
            if not line:
                break
            print(f"[{prefix}] {line.decode().rstrip()}")
    except Exception:
        pass  # Silently handle errors during shutdown


def check_node_modules():
    """Check if node_modules exists, run npm install if not"""
    node_modules = Path(WEB_DIR) / "node_modules"
    if not node_modules.exists():
        print_msg("Installing frontend dependencies...")
        result = subprocess.run(["npm", "install"], cwd=WEB_DIR, capture_output=False)
        if result.returncode != 0:
            print_error("npm install failed")
            sys.exit(1)
        print_success("Dependencies installed")


def start_frontend():
    """Start Vite dev server"""
    print_msg("Starting Vite dev server...")

    # Check dependencies
    check_node_modules()

    # Start Vite
    if sys.platform == "win32":
        # Windows needs shell=True for npm
        process = subprocess.Popen(
            "npm run dev",
            cwd=WEB_DIR,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            shell=True,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )
    else:
        process = subprocess.Popen(
            ["npm", "run", "dev"],
            cwd=WEB_DIR,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )

    # Start output thread
    thread = threading.Thread(target=stream_output, args=(process, "vite"), daemon=True)
    thread.start()

    # Wait for server to be ready
    time.sleep(2)

    return process


def start_backend():
    """Start Rust backend"""
    print_msg("Building Rust backend...")

    # Build backend
    build_result = subprocess.run(
        ["cargo", "build", "--features", "dev-server"], capture_output=True, text=True
    )

    if build_result.returncode != 0:
        print_error("Cargo build failed")
        print(build_result.stderr)
        return None

    print_msg("Starting Rust backend...")

    # Start backend
    if sys.platform == "win32":
        process = subprocess.Popen(
            "cargo run --features dev-server -- serve",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            shell=True,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )
    else:
        process = subprocess.Popen(
            ["cargo", "run", "--features", "dev-server", "--", "serve"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )

    # Start output thread
    thread = threading.Thread(
        target=stream_output, args=(process, "backend"), daemon=True
    )
    thread.start()

    # Wait for server to be ready
    time.sleep(2)

    return process


def cmd_start():
    """Start both frontend and backend"""
    print_msg("Starting development environment...")

    # Check if already running
    pids = read_pids()
    if pids:
        print_error("Development environment already running")
        print_msg(f"Frontend PID: {pids['frontend']}, Backend PID: {pids['backend']}")
        print_msg("Run 'python develop.py stop' first")
        sys.exit(1)

    # Reset shutdown flag
    _shutdown_flag.clear()

    frontend_process = None
    backend_process = None

    try:
        # Start frontend
        frontend_process = start_frontend()
        if frontend_process.poll() is not None:
            print_error("Vite failed to start")
            sys.exit(1)

        # Start backend
        backend_process = start_backend()
        if backend_process is None or backend_process.poll() is not None:
            print_error("Backend failed to start")
            kill_process(frontend_process.pid)
            sys.exit(1)

        # Save PIDs
        write_pids(frontend_process.pid, backend_process.pid)

        print_success("Development environment ready")
        print_msg("")
        print_msg(f"  Frontend: http://localhost:{VITE_PORT}")
        print_msg(f"  Backend:  http://localhost:{BACKEND_PORT}")
        print_msg("")
        print_msg("Press Ctrl+C to stop both servers")

        # Wait for Ctrl+C
        try:
            while True:
                time.sleep(1)
                # Check if processes are still alive
                if frontend_process.poll() is not None:
                    print_error("Frontend process died")
                    break
                if backend_process.poll() is not None:
                    print_error("Backend process died")
                    break
        except KeyboardInterrupt:
            print_msg("\nShutting down...")

    except KeyboardInterrupt:
        print_msg("\nShutting down...")
    except Exception as e:
        print_error(f"Failed to start: {e}")
    finally:
        # Signal output threads to stop
        _shutdown_flag.set()

        # Cleanup processes
        if frontend_process:
            kill_process(frontend_process.pid)
        if backend_process:
            kill_process(backend_process.pid)

        # Wait briefly for threads to finish
        time.sleep(0.5)

        # Remove PID file
        if os.path.exists(PID_FILE):
            os.remove(PID_FILE)

        # Restore terminal on Windows
        if sys.platform == "win32":
            try:
                # Flush stdout/stderr
                sys.stdout.flush()
                sys.stderr.flush()
                # Reset console mode on Windows
                os.system("")
            except Exception:
                # Intentionally ignore failures while restoring the Windows console
                pass

        print_success("Stopped")


def cmd_restart():
    """Restart backend only"""
    print_msg("Restarting backend...")

    pids = read_pids()
    if not pids:
        print_error("Development environment not running")
        print_msg("Run 'python develop.py start' first")
        sys.exit(1)

    # Kill backend
    print_msg(f"Stopping backend (PID {pids['backend']})...")
    kill_process(pids["backend"])
    time.sleep(1)

    # Start backend
    backend_process = start_backend()
    if backend_process is None or backend_process.poll() is not None:
        print_error("Backend failed to restart")
        sys.exit(1)

    # Update PIDs
    write_pids(pids["frontend"], backend_process.pid)

    print_success("Backend restarted")
    print_msg(f"Backend:  http://localhost:{BACKEND_PORT}")


def cmd_stop():
    """Stop both frontend and backend"""
    print_msg("Stopping development environment...")

    pids = read_pids()
    if not pids:
        print_error("Development environment not running")
        sys.exit(1)

    # Kill both processes
    print_msg(f"Stopping frontend (PID {pids['frontend']})...")
    kill_process(pids["frontend"])

    print_msg(f"Stopping backend (PID {pids['backend']})...")
    kill_process(pids["backend"])

    # Remove PID file
    if os.path.exists(PID_FILE):
        os.remove(PID_FILE)

    print_success("Stopped")


def main():
    if len(sys.argv) < 2:
        print("Usage: python develop.py <command>")
        print("")
        print("Commands:")
        print("  start    - Start both frontend and backend")
        print("  restart  - Restart backend only")
        print("  stop     - Stop both frontend and backend")
        sys.exit(1)

    command = sys.argv[1]

    if command == "start":
        cmd_start()
    elif command == "restart":
        cmd_restart()
    elif command == "stop":
        cmd_stop()
    else:
        print_error(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
