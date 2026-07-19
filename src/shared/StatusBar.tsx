import { useNoticeStore } from "./notices";

export function StatusBar() {
  const notice = useNoticeStore((state) => state.notice);

  return (
    <footer aria-live="polite" className="status-bar">
      <span className={`status-dot ${notice.kind}`} />
      <span>{notice.message}</span>
      <span className="status-spacer" />
      <span>No API key</span>
    </footer>
  );
}
