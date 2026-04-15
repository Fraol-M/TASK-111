-- Add `reviewer` as a first-class user_role. The prompt enumerates reviewers
-- distinctly from evaluators; the original enum collapsed the two concepts
-- which prevented authorization policy from distinguishing review authority
-- from evaluation authority. Reviewer is typically granted narrower rights
-- (read + approve/reject completed evaluations) while evaluator performs the
-- assessment work itself.
ALTER TYPE user_role ADD VALUE IF NOT EXISTS 'reviewer';
