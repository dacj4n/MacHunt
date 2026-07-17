import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export function CustomSelect<T extends string>({
  value,
  options,
  onChange,
  title,
  disabled,
  triggerClassName = "custom-select-trigger",
  style,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void | Promise<void>;
  title?: string;
  disabled?: boolean;
  triggerClassName?: string;
  style?: React.CSSProperties;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [panelStyle, setPanelStyle] = useState<React.CSSProperties>({});
  const panelRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    const handleMouseDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        panelRef.current?.contains(target) ||
        triggerRef.current?.contains(target)
      ) {
        return;
      }
      setIsOpen(false);
    };
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [isOpen]);

  // Calculate panel position relative to viewport
  const updatePosition = () => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    setPanelStyle({
      position: "fixed",
      top: rect.bottom + 8,
      left: rect.left,
      width: rect.width,
      zIndex: 9999,
    });
  };

  useEffect(() => {
    if (isOpen) {
      updatePosition();
      window.addEventListener("resize", updatePosition);
      window.addEventListener("scroll", updatePosition, true);
      return () => {
        window.removeEventListener("resize", updatePosition);
        window.removeEventListener("scroll", updatePosition, true);
      };
    }
  }, [isOpen]);

  const selectedLabel =
    options.find((o) => o.value === value)?.label ?? "";

  return (
    <div className="custom-select" style={style}>
      <button
        ref={triggerRef}
        type="button"
        className={triggerClassName}
        title={title}
        disabled={disabled}
        onClick={() => {
          if (disabled) return;
          setIsOpen((prev) => !prev);
        }}
      >
        {selectedLabel}
      </button>
      {isOpen && !disabled && createPortal(
        <div className="custom-select-panel" ref={panelRef} style={panelStyle}>
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              className={`custom-select-item ${opt.value === value ? "active" : ""}`}
              onClick={() => {
                void onChange(opt.value);
                setIsOpen(false);
              }}
            >
              <span className="custom-select-check">
                {opt.value === value ? "✓" : ""}
              </span>
              <span>{opt.label}</span>
            </button>
          ))}
        </div>,
        document.body
      )}
    </div>
  );
}
