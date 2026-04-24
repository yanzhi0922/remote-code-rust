import React from 'react';
import { X, Zap } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface SkillInfo {
  name: string;
  source: string;
  description?: string;
  tokenEstimate?: number;
}

type Props = {
  skills: SkillInfo[];
  onExit: () => void;
  onSelectSkill?: (skillName: string) => void;
};

function getSourceTitle(source: string): string {
  if (source === 'plugin') return 'Plugin skills';
  if (source === 'mcp') return 'MCP skills';
  const capitalized = source.charAt(0).toUpperCase() + source.slice(1);
  return `${capitalized} skills`;
}

export function SkillsMenu({ skills, onExit, onSelectSkill }: Props): React.ReactElement {
  const grouped = skills.reduce<Record<string, SkillInfo[]>>((acc, skill) => {
    const key = skill.source;
    if (!acc[key]) acc[key] = [];
    acc[key].push(skill);
    return acc;
  }, {});

  return (
    <div
      data-testid="skills-menu"
      className="rounded-lg border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="flex items-center justify-between border-b border-gray-200 px-4 py-3 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Zap className="h-5 w-5 text-cyan-500" />
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Skills
          </h3>
        </div>
        <button
          data-testid="skills-close-btn"
          aria-label="Close"
          onClick={onExit}
          className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          <X className="h-5 w-5" />
        </button>
      </div>

      <div className="p-4">
        {skills.length === 0 ? (
          <div className="flex flex-col gap-2">
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Create skills in .claude/skills/ or ~/.claude/skills/
            </p>
            <p className="text-sm italic text-gray-400 dark:text-gray-500">
              Esc to close
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {Object.entries(grouped).map(([source, sourceSkills]) => (
              <div key={source}>
                <h4 className="mb-2 text-sm font-semibold text-gray-700 dark:text-gray-300">
                  {getSourceTitle(source)}
                </h4>
                <div className="flex flex-col gap-1">
                  {sourceSkills.map((skill) => (
                    <button
                      key={skill.name}
                      data-testid={`skill-item-${skill.name}`}
                      className={cn(
                        'flex items-center justify-between rounded-md px-3 py-2 text-left text-sm',
                        'hover:bg-gray-50 dark:hover:bg-gray-700/50',
                        'transition-colors',
                      )}
                      onClick={() => onSelectSkill?.(skill.name)}
                    >
                      <span className="font-medium text-gray-900 dark:text-gray-100">
                        {skill.name}
                      </span>
                      <div className="flex items-center gap-2">
                        {skill.description && (
                          <span className="text-xs text-gray-500 dark:text-gray-400">
                            {skill.description}
                          </span>
                        )}
                        {skill.tokenEstimate != null && (
                          <span className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-700 dark:text-gray-400">
                            ~{skill.tokenEstimate} tokens
                          </span>
                        )}
                      </div>
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
