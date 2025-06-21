import { setCookie, deleteCookie } from './cookies';

/**
 * Validates a password by attempting to access a protected endpoint
 * @param password The password to validate
 * @returns Promise that resolves to true if valid, false otherwise
 */
export async function validatePassword(password: string): Promise<boolean> {
  try {
    const response = await fetch('/api/sites', {
      headers: {
        Authorization: password
      }
    });
    
    return response.ok;
  } catch (error) {
    console.error('Password validation error:', error);
    return false;
  }
}

/**
 * Stores the password in a cookie if validation succeeds
 * @param password The password to store
 * @returns Promise that resolves to true if validation succeeded, false otherwise
 */
export async function loginWithPassword(password: string): Promise<boolean> {
  const isValid = await validatePassword(password);
  
  if (isValid) {
    setCookie('password', password, 7); // Store for 7 days
    return true;
  }
  
  // Ensure any existing cookie is cleared on failed login
  deleteCookie('password');
  return false;
}