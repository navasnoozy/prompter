import { useNoticeStore } from "./notices";

export function StatusBar() {
  const notice = useNoticeStore((state) => state.notice);

  return (
    <footer className="status-bar">
      <span aria-hidden="true" className={`status-dot ${notice.kind}`} />
      <span
        aria-atomic="true"
        aria-live={notice.kind === "error" ? "assertive" : "polite"}
        role={notice.kind === "error" ? "alert" : "status"}
      >
        {notice.message}
      </span>
      <span className="status-spacer" />
      <span aria-label="Prompter does not require an API key">No API key</span>
    </footer>
  );
}
