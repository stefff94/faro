import type { ReactNode, RefObject } from "react";

export function DrawerPanel({
  open, pill, panel, onEnter, onLeave, onToggle, rootRef,
}: {
  open: boolean; pill: ReactNode; panel: ReactNode;
  onEnter: () => void; onLeave: () => void; onToggle: () => void;
  rootRef?: RefObject<HTMLDivElement | null>;
}) {
  return (
    <div className="drawer" ref={rootRef} onMouseEnter={onEnter} onMouseLeave={onLeave}>
      {open ? (
        <div className="panel" onClick={(e) => e.stopPropagation()}>{panel}</div>
      ) : (
        <div onClick={onToggle}>{pill}</div>
      )}
    </div>
  );
}
