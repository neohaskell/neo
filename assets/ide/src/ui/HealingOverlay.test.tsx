import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { HealingOverlay } from './HealingOverlay'

describe('HealingOverlay', () => {
  it('renders the default healing message and a spinner', () => {
    const { container } = render(<HealingOverlay />)
    expect(screen.getByText(/healing event model/i)).toBeInTheDocument()
    expect(container.querySelector('.animate-spin')).not.toBeNull()
  })

  it('uses a custom message when supplied', () => {
    render(<HealingOverlay message="Custom status" />)
    expect(screen.getByText('Custom status')).toBeInTheDocument()
  })

  it('marks itself as a live status region for screen readers', () => {
    render(<HealingOverlay />)
    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('shows the log scroller with a placeholder when no lines have arrived', () => {
    render(<HealingOverlay log={[]} />)
    const scroller = screen.getByTestId('heal-log')
    expect(scroller).toBeInTheDocument()
    expect(scroller.textContent).toMatch(/reasoning, tool calls, and results/i)
    expect(screen.getByText(/waiting for the agent/i)).toBeInTheDocument()
  })

  it('renders raw stderr lines verbatim', () => {
    render(
      <HealingOverlay
        log={[{ stream: 'stderr', line: 'INFO: starting' }]}
      />,
    )
    const raw = screen.getByTestId('heal-event-raw')
    expect(raw.textContent).toContain('INFO: starting')
  })

  it('renders thinking deltas as a "thinking" card with the streamed text', () => {
    render(
      <HealingOverlay
        log={[
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_delta',
                delta: { type: 'thinking_delta', thinking: 'Let me look at the file' },
              },
            }),
          },
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_delta',
                delta: { type: 'thinking_delta', thinking: ' and decide what to do.' },
              },
            }),
          },
        ]}
      />,
    )
    const thinking = screen.getByTestId('heal-event-thinking')
    // Fragmented deltas merged into one card.
    expect(thinking.textContent).toContain(
      'Let me look at the file and decide what to do.',
    )
  })

  it('renders a tool_use card with the tool name and accumulated JSON input', () => {
    render(
      <HealingOverlay
        log={[
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_start',
                content_block: { type: 'tool_use', id: 't1', name: 'Read' },
              },
            }),
          },
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_delta',
                delta: { type: 'input_json_delta', partial_json: '{"file_path":"' },
              },
            }),
          },
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_delta',
                delta: { type: 'input_json_delta', partial_json: '/tmp/x.json"}' },
              },
            }),
          },
        ]}
      />,
    )
    const tool = screen.getByTestId('heal-event-tool-use')
    expect(tool.textContent).toContain('Read')
    expect(tool.textContent).toContain('/tmp/x.json')
  })

  it('renders a tool_result card with a truncated preview', () => {
    render(
      <HealingOverlay
        log={[
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'user',
              message: {
                content: [
                  {
                    type: 'tool_result',
                    tool_use_id: 't1',
                    content: 'a'.repeat(800),
                  },
                ],
              },
            }),
          },
        ]}
      />,
    )
    const result = screen.getByTestId('heal-event-tool-result')
    expect(result.textContent).toMatch(/truncated/i)
  })

  it('skips the giant system/init payload and the rate_limit_event noise', () => {
    render(
      <HealingOverlay
        log={[
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'system',
              subtype: 'init',
              tools: Array.from({ length: 400 }, (_, i) => `tool_${i}`),
            }),
          },
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'rate_limit_event',
              rate_limit_info: { status: 'allowed' },
            }),
          },
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_delta',
                delta: { type: 'text_delta', text: 'visible reply' },
              },
            }),
          },
        ]}
      />,
    )
    // Only the visible reply renders; init + rate limit are dropped.
    expect(screen.queryByText(/tool_0/)).not.toBeInTheDocument()
    expect(screen.queryByText(/rate_limit_info/)).not.toBeInTheDocument()
    expect(screen.getByTestId('heal-event-text').textContent).toContain('visible reply')
  })

  it('falls back to a raw card when a line is not parseable JSON', () => {
    render(
      <HealingOverlay
        log={[{ stream: 'stdout', line: 'plain text from the subprocess' }]}
      />,
    )
    const raw = screen.getByTestId('heal-event-raw')
    expect(raw.textContent).toContain('plain text from the subprocess')
  })

  it('updates the step counter from "waiting" to the timeline length', () => {
    const { rerender } = render(<HealingOverlay log={[]} />)
    expect(screen.getByText(/waiting for the agent/i)).toBeInTheDocument()
    rerender(
      <HealingOverlay
        log={[
          {
            stream: 'stdout',
            line: JSON.stringify({
              type: 'stream_event',
              event: {
                type: 'content_block_delta',
                delta: { type: 'text_delta', text: 'hello' },
              },
            }),
          },
        ]}
      />,
    )
    expect(screen.queryByText(/waiting for the agent/i)).not.toBeInTheDocument()
    expect(screen.getByText(/1 step/)).toBeInTheDocument()
  })
})
