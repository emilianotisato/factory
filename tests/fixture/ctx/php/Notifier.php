<?php

namespace App\Services;

use App\Models\User;

interface Notifier
{
    public function notify(User $user, string $template): void;
}
