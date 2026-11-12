import { render, screen } from '@testing-library/react';
import App from './App';

test('renders three panels and title', async () => {
  render(<App />);
  expect(screen.getByText('Illy')).toBeInTheDocument();
  expect(screen.getByTestId('panel-sidebar')).toBeInTheDocument();
  expect(screen.getByTestId('panel-list')).toBeInTheDocument();
  expect(screen.getByTestId('panel-reader')).toBeInTheDocument();
});
