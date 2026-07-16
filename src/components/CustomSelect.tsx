import { useEffect, useRef, useState } from "react";

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
      {isOpen && !disabled && (
        <div className="custom-select-panel" ref={panelRef}>
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
        </div>
      )}
    </div>
  );
}
