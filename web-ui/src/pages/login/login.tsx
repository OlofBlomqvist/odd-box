import React, { useState, useEffect } from 'react';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { loginWithPassword } from '../../utils/auth';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";

export function LoginPage() {
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const navigate = useNavigate();
  const { redirect = '/' } = useSearch({ from: '/login' });
  
  // Try default password on component mount
  useEffect(() => {
    const tryDefaultPassword = async () => {
      setIsLoading(true);
      try {
        // Try the default password "changeme"
        const isValid = await loginWithPassword("changeme");
        if (isValid) {
          // If successful, navigate to the redirect URL
          navigate({ to: redirect as string });
        } else {
          // If default password doesn't work, just show login form
          setIsLoading(false);
        }
      } catch (err) {
        // In case of error, just show the login form
        setIsLoading(false);
      }
    };
    
    tryDefaultPassword();
  }, [navigate, redirect]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    try {
      const isValid = await loginWithPassword(password);
      
      if (isValid) {
        // Redirect to the original requested page or home
        navigate({ to: redirect as string });
      } else {
        setError('Invalid password. Please try again.');
      }
    } catch (err) {
      setError('An error occurred. Please try again.');
      console.error('Login error:', err);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="flex items-center justify-center min-h-screen p-4">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Log In</CardTitle>
          <CardDescription>
            Password authentication is required to access the application.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {error && (
            <div className="mb-4 p-3 rounded bg-red-100 border border-red-400 text-red-700 dark:bg-red-900/30 dark:text-red-400 dark:border-red-800">
              {error}
            </div>
          )}
          
          <form onSubmit={handleSubmit}>
            <div className="grid gap-4">
              <div className="grid gap-2">
                <label htmlFor="password" className="text-sm font-medium leading-none">
                  Password
                </label>
                <input
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                  placeholder="Enter password"
                  required
                />
              </div>
              
              <Button 
                type="submit" 
                disabled={isLoading}
                className="w-full"
              >
                {isLoading ? 'Validating...' : 'Login'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

export default LoginPage;