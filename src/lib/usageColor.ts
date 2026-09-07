export type UsageColorClass = 'text-success' | 'text-warning' | 'text-destructive'
export type UsageBorderClass = 'border-success/20' | 'border-warning/20' | 'border-destructive/20'

export function usageColorClass(percent: number): UsageColorClass {
  if (percent > 85) return 'text-destructive'
  if (percent > 60) return 'text-warning'
  return 'text-success'
}

// Kept as its own function (rather than deriving from usageColorClass with a
// string replace) so every class name is a literal Tailwind can find by
// scanning this file — a computed "border-" + color string wouldn't be.
export function usageBorderClass(percent: number): UsageBorderClass {
  if (percent > 85) return 'border-destructive/20'
  if (percent > 60) return 'border-warning/20'
  return 'border-success/20'
}
