import { useSubAgentStore } from '../../stores/subAgentStore';

/**
 * Header indicator showing active sub-agent count with badge
 * Click to toggle the detail panel
 */
export const SubAgentIndicator = () => {
  const activeRuns = useSubAgentStore((state) => state.activeRuns);
  const isPanelOpen = useSubAgentStore((state) => state.isPanelOpen);
  const togglePanel = useSubAgentStore((state) => state.togglePanel);

  const activeCount = activeRuns.size;
  const hasActive = activeCount > 0;

  return (
    <button
      onClick={togglePanel}
      className={`btn btn-ghost btn-sm gap-2 ${
        isPanelOpen ? 'btn-active' : ''
      }`}
      title="Sub-agent activity"
    >
      {/* Robot icon */}
      <svg
        xmlns="http://www.w3.org/2000/svg"
        className={`h-5 w-5 ${hasActive ? 'text-[#E8C236]' : 'text-base-content'}`}
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2z"
        />
      </svg>

      {/* Badge showing active count */}
      {hasActive && (
        <span className="badge badge-sm bg-[#E8C236] text-black border-none">
          {activeCount}
        </span>
      )}

      {/* Spinning indicator when active */}
      {hasActive && (
        <span className="loading loading-spinner loading-xs text-[#E8C236]"></span>
      )}
    </button>
  );
};
