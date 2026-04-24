import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserResourceUpdateMessage } from './UserResourceUpdateMessage';

describe('UserResourceUpdateMessage', () => {
  afterEach(cleanup);

  it('渲染创建类型的资源更新', () => {
    render(
      <UserResourceUpdateMessage
        resourceName="src/main.ts"
        resourceType="文件"
        kind="created"
      />,
    );
    expect(screen.getByText(/文件/)).toBeInTheDocument();
    expect(screen.getByText(/src\/main\.ts/)).toBeInTheDocument();
    expect(screen.getByText(/已创建/)).toBeInTheDocument();
  });

  it('渲染更新类型的资源更新', () => {
    render(
      <UserResourceUpdateMessage
        resourceName="config.json"
        resourceType="配置"
        kind="updated"
      />,
    );
    expect(screen.getByText(/已更新/)).toBeInTheDocument();
  });

  it('渲染删除类型的资源更新', () => {
    render(
      <UserResourceUpdateMessage
        resourceName="temp.log"
        resourceType="文件"
        kind="deleted"
      />,
    );
    expect(screen.getByText(/已删除/)).toBeInTheDocument();
  });

  it('渲染同步类型的资源更新', () => {
    render(
      <UserResourceUpdateMessage
        resourceName="project"
        resourceType="目录"
        kind="synced"
      />,
    );
    expect(screen.getByText(/已同步/)).toBeInTheDocument();
  });

  it('显示描述文本', () => {
    render(
      <UserResourceUpdateMessage
        resourceName="file.ts"
        resourceType="文件"
        kind="updated"
        description="添加了新函数"
      />,
    );
    expect(screen.getByText('添加了新函数')).toBeInTheDocument();
  });
});
