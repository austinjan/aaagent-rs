function App() {
  return (
    <div className="min-h-screen bg-base-100" data-theme="blackbear">
      <header className="navbar bg-neutral text-primary">
        <div className="flex-1">
          <a className="btn btn-ghost text-xl">aaagent-rs</a>
        </div>
      </header>

      <main className="container mx-auto p-4">
        <div className="alert alert-info">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            className="stroke-current shrink-0 w-6 h-6"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="2"
              d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            ></path>
          </svg>
          <span>Chat UI Foundation - Ready for implementation</span>
        </div>

        <div className="mt-8 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div className="card bg-base-200 shadow-xl">
            <div className="card-body">
              <h2 className="card-title text-primary">Frontend</h2>
              <p>React + TypeScript + Vite</p>
              <div className="badge badge-success">Active</div>
            </div>
          </div>

          <div className="card bg-base-200 shadow-xl">
            <div className="card-body">
              <h2 className="card-title text-secondary">Backend</h2>
              <p>Rust + axum (Coming soon)</p>
              <div className="badge badge-warning">Pending</div>
            </div>
          </div>

          <div className="card bg-base-200 shadow-xl">
            <div className="card-body">
              <h2 className="card-title text-accent">Features</h2>
              <ul className="list-disc list-inside text-sm">
                <li>SSE Streaming</li>
                <li>Tool Pairs</li>
                <li>Error Handling</li>
              </ul>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

export default App;
