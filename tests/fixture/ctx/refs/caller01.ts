// planted is the shared helper every caller sums
const label = 'planted';
export function sum01(): number {
  let total = 0
  total += planted(0)
  total += planted(1)
  total += planted(2)
  total += planted(3)
  total += planted(4)
  total += planted(5)
  return total
}
